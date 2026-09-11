// Windows OCR adapter adapted from MORT/OcrApi/WindowOcr/WindowOcr.cs,
// kmonkeyhead/MORT revision ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a (MIT).
// Copyright (c) 2024 몽키해드. See LICENSE-MORT.txt.
// Neeko: async Task API, explicit language failure (no silent fallback), bounded
// in-memory bitmaps and deterministic disposal; no TTS or MORT UI dependencies.
using Windows.Globalization;
using Windows.Graphics.Imaging;
using Windows.Media.Ocr;
using Windows.Storage.Streams;

namespace Neeko.ScreenOcr;

internal sealed class WindowsOcr
{
    private OcrEngine? engine;
    private string language = "";

    public static OcrLanguage[] Languages() => OcrEngine.AvailableRecognizerLanguages
        .Select(item => new OcrLanguage(item.LanguageTag, item.DisplayName, "windows", true, item.LanguageTag)).ToArray();

    public async Task<string> Recognize(Bitmap image, string code)
        => string.Join(Environment.NewLine, (await RecognizeBlocks(image, code)).Select(b => b.Text));

    public async Task<OcrBlock[]> RecognizeBlocks(Bitmap image, string code)
    {
        if (engine == null || language != code)
        {
            if (string.IsNullOrWhiteSpace(code) || !OcrEngine.IsLanguageSupported(new Language(code)))
                throw new InvalidOperationException("El idioma OCR no está instalado. Agregá su reconocimiento óptico en Idioma y región de Windows y volvé a comprobar.");
            engine = OcrEngine.TryCreateFromLanguage(new Language(code))
                ?? throw new InvalidOperationException("Windows no pudo iniciar el motor OCR para ese idioma.");
            language = code;
        }
        var scale = Math.Min(1.0, (double)OcrEngine.MaxImageDimension / Math.Max(image.Width, image.Height));
        using var resized = new Bitmap(image, new Size(Math.Max(1, (int)(image.Width * scale)), Math.Max(1, (int)(image.Height * scale))));
        using var png = new MemoryStream();
        resized.Save(png, System.Drawing.Imaging.ImageFormat.Png);
        using var stream = new InMemoryRandomAccessStream();
        using (var writer = new DataWriter(stream.GetOutputStreamAt(0)))
        {
            writer.WriteBytes(png.ToArray());
            await writer.StoreAsync();
            await writer.FlushAsync();
        }
        stream.Seek(0);
        var decoder = await BitmapDecoder.CreateAsync(stream);
        using var bitmap = await decoder.GetSoftwareBitmapAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied);
        var result = await engine.RecognizeAsync(bitmap);
        return result.Lines.Where(line => line.Words.Count > 0).Select(line => {
            var left = line.Words.Min(w => w.BoundingRect.Left);
            var top = line.Words.Min(w => w.BoundingRect.Top);
            var right = line.Words.Max(w => w.BoundingRect.Right);
            var bottom = line.Words.Max(w => w.BoundingRect.Bottom);
            return new OcrBlock(line.Text.Trim(), left / scale, top / scale, (right - left) / scale, (bottom - top) / scale);
        }).ToArray();
    }
}
