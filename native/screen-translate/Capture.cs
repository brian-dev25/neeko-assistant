using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Neeko.ScreenOcr;

internal static class Capture
{
    private delegate bool EnumProc(nint window, nint state);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, nint state);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(nint window);
    [DllImport("user32.dll")] private static extern bool IsIconic(nint window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(nint window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(nint window, out NativeRect rect);
    [DllImport("dwmapi.dll")] private static extern int DwmGetWindowAttribute(nint window,int attribute,out NativeRect rect,int size);
    private static WindowCapture? capture;
    private static nint captureWindow;
    public static void Close() {capture?.Dispose();capture=null;captureWindow=0;}
    [StructLayout(LayoutKind.Sequential)] private struct NativeRect { public int Left, Top, Right, Bottom; }
    internal record WindowInfo(string Id, string Title, int X, int Y, int Width, int Height);
    public static WindowInfo[] Windows()
    {
        var result = new List<WindowInfo>();
        EnumWindows((window, _) => {
            var title = new StringBuilder(512);
            if (IsWindowVisible(window) && !IsIconic(window) && GetWindowText(window, title, title.Capacity) > 0 && GetWindowRect(window, out var r)
                && r.Right - r.Left >= 20 && r.Bottom - r.Top >= 20)
                result.Add(new(window.ToInt64().ToString(), title.ToString(), r.Left, r.Top, r.Right-r.Left, r.Bottom-r.Top));
            return true;
        }, 0);
        return result.ToArray();
    }
    public static (Bitmap Image, Region Region) Read(CaptureRequest request)
    {
        var region = request.Region;
        if (request.FollowMouse)
        {
            var point = Cursor.Position; var screen = Screen.FromPoint(point).Bounds;
            region = region with { X = Math.Clamp(point.X - region.Width/2, screen.Left, Math.Max(screen.Left, screen.Right-region.Width)),
                Y = Math.Clamp(point.Y - region.Height/2, screen.Top, Math.Max(screen.Top, screen.Bottom-region.Height)) };
        }
        if (!string.IsNullOrEmpty(request.WindowId))
        {
            if (!long.TryParse(request.WindowId, out var id) || id <= 0) throw new ArgumentException("Ventana inválida.");
            var handle = (nint)id;
            if (IsIconic(handle) || !GetWindowRect(handle, out var rect)) throw new InvalidOperationException("La ventana se cerró o está minimizada.");
            var width = rect.Right-rect.Left; var height = rect.Bottom-rect.Top;
            if (width < 20 || height < 20 || width > 8192 || height > 8192 || (long)width*height > 33_000_000) throw new ArgumentException("Ventana demasiado grande.");
            var local = new Rectangle(region.X-request.WindowX, region.Y-request.WindowY, region.Width, region.Height);
            if (!new Rectangle(0,0,width,height).Contains(local)) throw new ArgumentException("La región ya no cabe dentro de la ventana. Volvé a seleccionarla.");
            if(captureWindow!=handle) {Close();capture=new WindowCapture(handle);captureWindow=handle;}
            using var full=capture!.Read().GetAwaiter().GetResult();
            var actual=region with { X=rect.Left+local.X,Y=rect.Top+local.Y };
            if(DwmGetWindowAttribute(handle,9,out var captureBounds,Marshal.SizeOf<NativeRect>())==0)
                local.Offset(rect.Left-captureBounds.Left,rect.Top-captureBounds.Top);
            if(!new Rectangle(0,0,full.Width,full.Height).Contains(local))throw new InvalidOperationException("La región está fuera del contenido capturable de la ventana.");
            return (full.Clone(local,PixelFormat.Format32bppArgb), actual);
        }
        region.Validate();
        var image = new Bitmap(region.Width,region.Height,PixelFormat.Format32bppArgb);
        try { using var graphics = Graphics.FromImage(image); graphics.CopyFromScreen(region.X,region.Y,0,0,image.Size,CopyPixelOperation.SourceCopy); }
        catch { image.Dispose(); throw; }
        return (image,region);
    }
    public static byte[] Pixels(Bitmap image)
    {
        var data = image.LockBits(new Rectangle(0,0,image.Width,image.Height),ImageLockMode.ReadOnly,PixelFormat.Format32bppArgb);
        try { var bytes = new byte[Math.Abs(data.Stride)*data.Height]; Marshal.Copy(data.Scan0,bytes,0,bytes.Length); return bytes; }
        finally { image.UnlockBits(data); }
    }
    public static string Fingerprint(Bitmap image) => Convert.ToHexString(SHA256.HashData(Pixels(image)));
    public static Bitmap Prepare(Bitmap image, CaptureRequest request)
    {
        if (request.Scale < 1 || request.Scale > 3 || request.Threshold < 0 || request.Threshold > 255) throw new ArgumentException("Ajustes de imagen inválidos.");
        var scale = Math.Min(request.Scale, Math.Min(4096d/image.Width,4096d/image.Height));
        if (image.Width*image.Height*scale*scale > 8_000_000) scale = Math.Sqrt(8_000_000d/(image.Width*image.Height));
        var output = new Bitmap(Math.Max(1,(int)(image.Width*scale)),Math.Max(1,(int)(image.Height*scale)),PixelFormat.Format32bppArgb);
        using (var graphics = Graphics.FromImage(output)) { graphics.DrawImage(image,new Rectangle(0,0,output.Width,output.Height)); }
        OcrImageFilters.Mask(output,request);
        if (request.Brightness is < -100 or > 100 || request.Contrast is < -100 or > 100)
            throw new ArgumentException("Brightness and contrast must be between -100 and 100.");
        if (request.Invert || request.Threshold > 0 || request.Grayscale || request.Brightness != 0 || request.Contrast != 0)
        {
            var bytes = Pixels(output);
            var contrast = Math.Pow((100d + request.Contrast) / 100d, 2);
            for (var i=0;i<bytes.Length;i+=4)
            {
                for (var channel=0;channel<3;channel++)
                    bytes[i+channel]=(byte)Math.Clamp((bytes[i+channel]-127.5)*contrast+127.5+request.Brightness*2.55,0,255);
                if (request.Grayscale) { var gray=(byte)((bytes[i]*11+bytes[i+1]*59+bytes[i+2]*30)/100); bytes[i]=bytes[i+1]=bytes[i+2]=gray; }
                if (request.Threshold > 0) { var gray=(bytes[i]*11+bytes[i+1]*59+bytes[i+2]*30)/100; bytes[i]=bytes[i+1]=bytes[i+2]=(byte)(gray >= request.Threshold ? 255:0); }
                if (request.Invert) { bytes[i]=(byte)(255-bytes[i]); bytes[i+1]=(byte)(255-bytes[i+1]); bytes[i+2]=(byte)(255-bytes[i+2]); }
            }
            var data=output.LockBits(new Rectangle(0,0,output.Width,output.Height),ImageLockMode.WriteOnly,PixelFormat.Format32bppArgb);
            try { Marshal.Copy(bytes,0,data.Scan0,bytes.Length); } finally { output.UnlockBits(data); }
        }
        if(request.Erode) OcrImageFilters.Erode(output);
        return output;
    }
}
