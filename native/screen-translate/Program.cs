using System.Text;
using System.Text.Json;
using System.Runtime.InteropServices;

namespace Neeko.ScreenOcr;

internal record Region(int X, int Y, int Width, int Height)
{
    public Rectangle Bounds => new(X, Y, Width, Height);
    public void Validate()
    {
        if (Width < 20 || Height < 20 || Width > 4096 || Height > 4096 || (long)Width * Height > 8_000_000
            || X < -65536 || Y < -65536 || X > 65536 || Y > 65536)
            throw new ArgumentException("Elegí una región entre 20 y 4096 píxeles por lado, de hasta 8 megapíxeles.");
        if (!SystemInformation.VirtualScreen.Contains(Bounds) || !Screen.AllScreens.Any(s => s.Bounds.IntersectsWith(Bounds)))
            throw new ArgumentException("La región está fuera de los monitores actuales. Volvé a seleccionarla.");
    }
}

internal record CaptureRequest(Region Region, string Source, string Engine = "windows", double Scale = 1, int Threshold = 0, bool Invert = false,
    bool Force = false, string WindowId = "", int WindowX = 0, int WindowY = 0, bool FollowMouse = false, Region[]? Exclusions = null, bool ClipboardInput = false,
    int Brightness = 0, int Contrast = 0, bool Grayscale = false, bool TesseractEnglishFilter = true,
    bool VerticalText = false, bool AutoColors = false, string ColorFilter = "none", string FilterColor = "#ffffff", int ColorTolerance = 20, bool Erode = false);

internal static class Program
{
    private static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, PropertyNameCaseInsensitive = true };
    private static void Send(object value) { Console.WriteLine(JsonSerializer.Serialize(value, Json)); Console.Out.Flush(); }

    [STAThread]
    private static int Main(string[] args)
    {
        Console.InputEncoding = Encoding.UTF8;
        Console.OutputEncoding = new UTF8Encoding(false);
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        Environment.SetEnvironmentVariable("OMP_THREAD_LIMIT", "1");
        try
        {
            switch (args.SingleOrDefault())
            {
                case "--self-test-image":
                    using (var sample = new Bitmap(20,20))
                    {
                        using (var graphics = Graphics.FromImage(sample)) graphics.Clear(Color.FromArgb(60,120,180));
                        var request = new CaptureRequest(new Region(0,0,20,20),"auto");
                        using var unchanged = Capture.Prepare(sample,request);
                        using var grayscale = Capture.Prepare(sample,request with {Grayscale=true});
                        using var brighter = Capture.Prepare(sample,request with {Brightness=20});
                        using var contrast = Capture.Prepare(sample,request with {Contrast=50});
                        using var inverted = Capture.Prepare(sample,request with {Invert=true});
                        var g=grayscale.GetPixel(10,10);
                        if(unchanged.GetPixel(10,10).R!=60 || g.R!=g.G || g.G!=g.B || brighter.GetPixel(10,10).R<=60
                            || contrast.GetPixel(10,10).R>=60 || inverted.GetPixel(10,10).R!=195)
                            throw new InvalidOperationException("Image preprocessing regression");
                        Send(new {ok=true,tests="identity, grayscale, brightness, contrast, inversion"});
                    }
                    break;
                case "--languages":
                    Send(new { languages = WindowsOcr.Languages().Concat(OcrModels.Languages()).Append(OneOcr.Language()), windows = Capture.Windows() });
                    break;
                case "--install":
                    var install = JsonSerializer.Deserialize<Dictionary<string,string>>(Console.ReadLine() ?? "{}", Json)!;
                    OcrModels.Install(install["source"]).GetAwaiter().GetResult();
                    Send(new { languages = WindowsOcr.Languages().Concat(OcrModels.Languages()).Append(OneOcr.Language()), windows = Capture.Windows() });
                    break;
                case "--clipboard":
                    Send(new { text = Clipboard.ContainsText() ? Clipboard.GetText() : "" });
                    break;
                case "--self-test-oneocr":
                case "--self-test-tesseract":
                    using (var testImage = new Bitmap(1000,180))
                    using (var graphics = Graphics.FromImage(testImage))
                    using (var font = new Font("Arial",38))
                    using (var tess = new TesseractOcr())
                    using (var one = new OneOcr())
                    {
                        graphics.Clear(Color.White); graphics.DrawString("HELLO WORLD 12345",font,Brushes.Black,20,40);
                        var blocks=args[0]=="--self-test-oneocr" ? one.Recognize(testImage) : tess.Recognize(testImage,"eng");
                        if (!blocks.Any(b=>b.Text.Contains("12345"))) throw new InvalidOperationException("Tesseract no reconoció la imagen sintética.");
                        Send(new { ok=true, blocks });
                    }
                    break;
                case "--self-test-spatial":
                    using(var form = new Form { Width=1200, Height=220, ShowInTaskbar=false, BackColor=Color.White })
                    using(var font = new Font("Arial", 24))
                    {
                        const string phrase = "las habilidades de programacion que debi aprender antes";
                        form.Paint += (_, e) => e.Graphics.DrawString(phrase, font, Brushes.Black, 24, 32);
                        form.Shown += async (_, _) => {
                            try {
                                var origin = form.PointToScreen(Point.Empty);
                                var info = Capture.Windows().Single(w => w.Id == form.Handle.ToInt64().ToString());
                                var region = new Region(origin.X, origin.Y, form.ClientSize.Width, form.ClientSize.Height);
                                var request = new CaptureRequest(region, "es-ES", WindowId:info.Id, WindowX:info.X, WindowY:info.Y);
                                var captured = await Task.Run(() => Capture.Read(request));
                                using var bitmap = captured.Image;
                                var blocks = await new WindowsOcr().RecognizeBlocks(bitmap, "es-ES");
                                if (!blocks.Any(b => b.Text.Contains("habilidades"))) throw new InvalidOperationException("Spanish OCR failed");
                                var first = blocks.First();
                                if (Math.Abs(first.X - 24) > 12 || Math.Abs(first.Y - 32) > 16 || captured.Region != region)
                                    throw new InvalidOperationException("OCR coordinate mapping failed");
                                Send(new { ok=true, backend="WGC + Windows OCR", region=captured.Region, blocks });
                            } catch(Exception error) { Send(new { error=error.GetBaseException().Message }); }
                            finally { Capture.Close(); form.Close(); }
                        };
                        form.Text = "Neeko synthetic spatial OCR test";
                        form.ShowDialog();
                    }
                    break;
                case "--self-test-capture":
                    using(var form=new Form {Text="Neeko OCR · prueba sintética",Width=1000,Height=240,ShowInTaskbar=false,BackColor=Color.White})
                    using(var font=new Font("Arial",38))
                    {
                        form.Paint+=(_,e)=>e.Graphics.DrawString("HELLO WORLD 12345",font,Brushes.Black,20,40);
                        form.Shown+=async (_,_)=> {
                            try {
                                using var capture=new WindowCapture(form.Handle);
                                using var bitmap=await capture.Read();
                                using var tess=new TesseractOcr();
                                var blocks=tess.Recognize(bitmap,"eng");
                                if(!blocks.Any(b=>b.Text.Contains("12345")))throw new InvalidOperationException("La captura WGC no reconoció la ventana sintética.");
                                Send(new {ok=true,backend="Windows Graphics Capture",blocks});
                            } catch(Exception error){Send(new {error=error.GetBaseException().Message});}
                            finally {form.Close();}
                        };
                        form.ShowDialog();
                    }
                    break;
                case "--select":
                    using (var selector = new RegionSelector())
                    {
                        // Closing the parent's stdin pipe also closes selection after a crash.
                        // The normal parent keeps this pipe open until the selection finishes.
                        selector.Shown += (_, _) => _ = Task.Run(async () =>
                        {
                            await Console.In.ReadLineAsync();
                            if (!selector.IsDisposed && selector.IsHandleCreated)
                            {
                                try { selector.BeginInvoke(() => selector.Close()); }
                                catch (InvalidOperationException) { }
                            }
                        });
                        selector.ShowDialog();
                        Send(new { region = selector.Selection, cancelled = selector.Selection == null });
                    }
                    break;
                case "--serve":
                    Serve().GetAwaiter().GetResult();
                    break;
                case "--self-test":
                    SelfTest().GetAwaiter().GetResult();
                    break;
                default: throw new ArgumentException("Modo de motor no válido.");
            }
            return 0;
        }
        catch (Exception error) { Send(new { error = error.GetBaseException().Message }); return 1; }
    }

    private static async Task Serve()
    {
        var ocr = new WindowsOcr();
        using var tess = new TesseractOcr();
        using var one = new OneOcr();
        var fingerprints = new Dictionary<string,string>();
        while (await Console.In.ReadLineAsync() is { } line)
        {
            try
            {
                if (line.Length > 16384) throw new ArgumentException("Solicitud demasiado grande.");
                var request = JsonSerializer.Deserialize<CaptureRequest>(line, Json) ?? throw new ArgumentException("Solicitud vacía.");
                if(request.ClipboardInput)
                {
                    var clipboard="";
                    var reader=new Thread(()=> {try {if(Clipboard.ContainsText())clipboard=Clipboard.GetText();} catch(ExternalException) {}});
                    reader.SetApartmentState(ApartmentState.STA);reader.Start();reader.Join();
                    if(clipboard.Length>6000)throw new ArgumentException("El portapapeles supera el límite de 6000 caracteres.");
                    Send(new OcrFrame(clipboard,[],660,240,new Region(0,0,660,240)));continue;
                }
                if (request.Region.Width < 20 || request.Region.Height < 20 || request.Region.Width > 4096 || request.Region.Height > 4096 || (long)request.Region.Width*request.Region.Height > 8_000_000)
                    throw new ArgumentException("La región es inválida.");
                var captured = Capture.Read(request);
                using var image = captured.Image;
                if (request.Exclusions is { Length: > 20 }) throw new ArgumentException("Demasiadas exclusiones.");
                using (var graphics=Graphics.FromImage(image))
                    foreach (var exclusion in request.Exclusions ?? []) graphics.FillRectangle(Brushes.White,exclusion.X-captured.Region.X,exclusion.Y-captured.Region.Y,exclusion.Width,exclusion.Height);
                var fingerprint = Capture.Fingerprint(image);
                var cacheKey = JsonSerializer.Serialize(request with { Force=false });
                if (!request.Force && fingerprints.TryGetValue(cacheKey,out var previous) && fingerprint == previous)
                { Send(new OcrFrame("",[],image.Width,image.Height,captured.Region,true)); continue; }
                using var prepared = Capture.Prepare(image,request);
                var blocks = request.Engine switch {
                    "windows" => await ocr.RecognizeBlocks(prepared,request.Source),
                    "tesseract" => tess.Recognize(prepared,request.Source,request.TesseractEnglishFilter,request.VerticalText),
                    "oneocr" => one.Recognize(prepared),
                    _ => throw new ArgumentException("Motor OCR desconocido.")
                };
                blocks=blocks.Take(100).Select(b=>b with { X=b.X*image.Width/prepared.Width, Y=b.Y*image.Height/prepared.Height,
                    Width=b.Width*image.Width/prepared.Width, Height=b.Height*image.Height/prepared.Height }).ToArray();
                if(request.AutoColors) blocks=blocks.Select(b=>OcrImageFilters.Colors(image,b)).ToArray();
                var text = string.Join(Environment.NewLine,blocks.Select(b=>b.Text));
                if (text.Length > 6000) throw new ArgumentException("Hay demasiado texto. Seleccioná una región más chica.");
                fingerprints[cacheKey]=fingerprint;
                if(fingerprints.Count>20) fingerprints.Clear();
                Send(new OcrFrame(text,blocks,image.Width,image.Height,captured.Region));
            }
            catch (Exception error) { Send(new { error = error.Message }); }
        }
        Capture.Close();
    }

    private static async Task SelfTest()
    {
        // Synthetic image only: this check never captures the user's desktop.
        using var bitmap = new Bitmap(1000, 180);
        using (var graphics = Graphics.FromImage(bitmap))
        using (var font = new Font("Arial", 38))
        {
            graphics.Clear(Color.White);
            graphics.DrawString("HELLO WORLD 12345", font, Brushes.Black, 20, 40);
        }
        var languages = Windows.Media.Ocr.OcrEngine.AvailableRecognizerLanguages;
        var code = languages.FirstOrDefault(l => l.LanguageTag.StartsWith("en"))?.LanguageTag
            ?? languages.FirstOrDefault()?.LanguageTag ?? throw new InvalidOperationException("No hay idiomas OCR instalados.");
        var text = await new WindowsOcr().Recognize(bitmap, code);
        if (!text.Contains("12345")) throw new InvalidOperationException("La prueba OCR sintética no reconoció el número esperado: " + text);
        foreach (var invalid in new[] { new Region(0, 0, 0, 10), new Region(0, 0, 4097, 20), new Region(int.MaxValue, 0, 30, 30) })
        {
            try { invalid.Validate(); throw new InvalidOperationException("Se aceptó geometría inválida."); }
            catch (ArgumentException) { }
        }
        Send(new { ok = true, language = code, text, tests = "synthetic-ocr, invalid-geometry" });
    }
}

internal sealed class RegionSelector : Form
{
    private Point? origin;
    private Rectangle selection;
    public Region? Selection { get; private set; }
    public RegionSelector()
    {
        AutoScaleMode = AutoScaleMode.None;
        FormBorderStyle = FormBorderStyle.None;
        StartPosition = FormStartPosition.Manual;
        Bounds = SystemInformation.VirtualScreen;
        BackColor = Color.Black;
        Opacity = .38;
        TopMost = true;
        ShowInTaskbar = false;
        DoubleBuffered = true;
        Cursor = Cursors.Cross;
        KeyPreview = true;
        KeyDown += (_, e) => { if (e.KeyCode == Keys.Escape) Close(); };
        MouseDown += (_, e) => { if (e.Button == MouseButtons.Left) { origin = e.Location; Capture = true; } };
        MouseMove += (_, e) =>
        {
            if (origin is not { } p) return;
            selection = Rectangle.FromLTRB(Math.Min(p.X, e.X), Math.Min(p.Y, e.Y), Math.Max(p.X, e.X), Math.Max(p.Y, e.Y));
            Invalidate();
        };
        MouseUp += (_, e) =>
        {
            if (origin == null || e.Button != MouseButtons.Left) return;
            Capture = false;
            origin = null;
            var region = new Region(Left + selection.X, Top + selection.Y, selection.Width, selection.Height);
            try { region.Validate(); Selection = region; Close(); }
            catch (ArgumentException) { selection = Rectangle.Empty; Invalidate(); }
        };
    }
    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        using var font = new Font("Segoe UI", 18);
        // Put instructions on the monitor containing the pointer (also supports negative coordinates).
        var screen = Screen.FromPoint(System.Windows.Forms.Cursor.Position).Bounds;
        e.Graphics.DrawString("Arrastrá sobre los diálogos · Esc: cancelar · Máximo 4096 px / 8 MP", font, Brushes.White,
            screen.Left - Left + 30, screen.Top - Top + 30);
        if (!selection.IsEmpty)
        {
            using var pen = new Pen(Color.Lime, 4);
            e.Graphics.DrawRectangle(pen, selection);
        }
    }
}
