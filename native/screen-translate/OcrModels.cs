using System.Globalization;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Tesseract;

namespace Neeko.ScreenOcr;

internal record OcrBlock(string Text, double X, double Y, double Width, double Height, string? Background = null, string? Foreground = null);
internal record OcrFrame(string Text, OcrBlock[] Blocks, int Width, int Height, Region Region, bool Unchanged = false);
internal record OcrLanguage(string Code, string Name, string Engine, bool Installed, string TranslationCode, long Size = 0);
internal record ModelEntry(string Code, string TranslationCode, string Sha, long Size,
    string Repository = "tessdata_fast", string? Revision = null, string? OcrCode = null);
internal record ModelCatalog(string Commit, ModelEntry[] Languages);

internal static class OcrModels
{
    private static readonly JsonSerializerOptions Json = new() { PropertyNameCaseInsensitive = true };
    public static readonly string DirectoryPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "neeko-assistant", "screen-translate", "tessdata");
    public static readonly ModelCatalog Catalog = LoadCatalog();
    private static ModelCatalog LoadCatalog()
    {
        var assembly = Assembly.GetExecutingAssembly();
        using var stream = assembly.GetManifestResourceStream(assembly.GetManifestResourceNames().Single(n => n.EndsWith("tessdata-catalog.json")))!;
        return JsonSerializer.Deserialize<ModelCatalog>(stream, Json)!;
    }
    public static string ModelPath(string code) => Path.Combine(DirectoryPath, code + ".traineddata");
    public static ModelEntry Get(string code) => Catalog.Languages.FirstOrDefault(l => l.Code == code && l.Code != "equ")
        ?? throw new ArgumentException("Idioma Tesseract desconocido.");
    private static string DisplayName(ModelEntry model)
    {
        try { return CultureInfo.GetCultureInfo(model.TranslationCode).DisplayName + ((model.OcrCode ?? model.Code).EndsWith("_vert") ? " · vertical" : "")
            + " · " + (model.Repository == "tessdata" ? "Normal" : model.Repository == "tessdata_best" ? "Precisión" : "Rápido") + " · " + model.Code; }
        catch { return model.Code; }
    }
    public static OcrLanguage[] Languages() => Catalog.Languages.Where(l => l.Code != "equ").Select(l =>
        new OcrLanguage(l.Code, DisplayName(l), "tesseract", File.Exists(ModelPath(l.Code)) && new FileInfo(ModelPath(l.Code)).Length == l.Size, l.TranslationCode, l.Size)).ToArray();

    public static async Task Install(string code)
    {
        var model = Get(code);
        using var client = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false }) { Timeout = TimeSpan.FromMinutes(2) };
        using var response = await client.GetAsync($"https://raw.githubusercontent.com/tesseract-ocr/{model.Repository}/{model.Revision ?? Catalog.Commit}/{model.OcrCode ?? model.Code}.traineddata", HttpCompletionOption.ResponseHeadersRead);
        response.EnsureSuccessStatusCode();
        Directory.CreateDirectory(DirectoryPath);
        var temporary = Path.Combine(DirectoryPath, $".{model.Code}.part");
        try
        {
            using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA1);
            hash.AppendData(Encoding.UTF8.GetBytes($"blob {model.Size}\0"));
            await using (var output = File.Create(temporary))
            await using (var input = await response.Content.ReadAsStreamAsync())
            {
                var buffer = new byte[65536]; long size = 0; int read;
                using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(2));
                while ((read = await input.ReadAsync(buffer, timeout.Token)) > 0)
                {
                    size += read;
                    if (size > model.Size || size > 200_000_000) throw new InvalidDataException("Paquete de idioma demasiado grande.");
                    hash.AppendData(buffer, 0, read);
                    await output.WriteAsync(buffer.AsMemory(0, read), timeout.Token);
                }
                if (size != model.Size || !Convert.ToHexString(hash.GetHashAndReset()).Equals(model.Sha, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("El paquete de idioma no coincide con el catálogo verificado.");
            }
            File.Move(temporary, ModelPath(code), true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}

internal sealed class TesseractOcr : IDisposable
{
    private TesseractEngine? engine;
    private string language = "";
    public OcrBlock[] Recognize(Bitmap image, string code, bool limitEnglishCharacters = true, bool verticalText = false)
    {
        var model=OcrModels.Get(code);
        var ocrCode=model.OcrCode ?? code;
        if (!File.Exists(OcrModels.ModelPath(code))) throw new InvalidOperationException("Descargá el idioma Tesseract desde el panel.");
        if (engine == null || language != code)
        {
            engine?.Dispose();
            engine = new TesseractEngine(OcrModels.DirectoryPath, code, EngineMode.LstmOnly);
            language = code;
        }
        using var stream = new MemoryStream();
        // MORT_CORE setTessdata limits the English alphabet before recognition.
        // Keep %, +, :, / as well: game stats use them. Never limit other languages.
        engine.SetVariable("tessedit_char_whitelist", ocrCode == "eng" && limitEnglishCharacters
            ? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,`'&?!()- %:+/"
            : "");
        image.Save(stream, System.Drawing.Imaging.ImageFormat.Png);
        using var pix = Pix.LoadFromMemory(stream.ToArray());
        using var page = engine.Process(pix, verticalText || ocrCode.EndsWith("_vert") ? PageSegMode.SingleBlockVertText : PageSegMode.Auto);
        using var iterator = page.GetIterator();
        var blocks = new List<OcrBlock>();
        iterator.Begin();
        do
        {
            var text = iterator.GetText(PageIteratorLevel.TextLine)?.Trim();
            if (!string.IsNullOrWhiteSpace(text) && iterator.TryGetBoundingBox(PageIteratorLevel.TextLine, out var rect))
                blocks.Add(new(text, rect.X1, rect.Y1, rect.Width, rect.Height));
        } while (iterator.Next(PageIteratorLevel.TextLine));
        return blocks.ToArray();
    }
    public void Dispose() => engine?.Dispose();
}
