using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace Neeko.ScreenOcr;

internal static class OcrImageFilters
{
    private static void Write(Bitmap image, byte[] pixels)
    {
        var data=image.LockBits(new Rectangle(0,0,image.Width,image.Height),ImageLockMode.WriteOnly,PixelFormat.Format32bppArgb);
        try { Marshal.Copy(pixels,0,data.Scan0,pixels.Length); }
        finally { image.UnlockBits(data); }
    }

    public static void Mask(Bitmap image, CaptureRequest request)
    {
        if (request.ColorFilter is not ("none" or "rgb" or "hsv") || request.ColorTolerance is < 0 or > 100)
            throw new ArgumentException("Filtro de color inválido.");
        if(request.ColorFilter=="none") return;
        if(request.FilterColor.Length!=7 || request.FilterColor[0]!='#' || !request.FilterColor[1..].All(Uri.IsHexDigit))
            throw new ArgumentException("Color de filtro inválido.");
        var target=ColorTranslator.FromHtml(request.FilterColor);
        var tolerance=request.ColorTolerance/100d;
        var bytes=Capture.Pixels(image);
        for(int i=0;i<bytes.Length;i+=4)
        {
            var color=Color.FromArgb(bytes[i+2],bytes[i+1],bytes[i]);
            bool match;
            if(request.ColorFilter=="rgb")
                match=Math.Max(Math.Abs(color.R-target.R),Math.Max(Math.Abs(color.G-target.G),Math.Abs(color.B-target.B)))<=255*tolerance;
            else
            {
                var h=Math.Abs(color.GetHue()-target.GetHue());h=Math.Min(h,360-h)/180;
                var max=Math.Max(color.R,Math.Max(color.G,color.B));var min=Math.Min(color.R,Math.Min(color.G,color.B));
                var tmax=Math.Max(target.R,Math.Max(target.G,target.B));var tmin=Math.Min(target.R,Math.Min(target.G,target.B));
                var saturation=max==0?0:(max-min)/(double)max;
                var targetSaturation=tmax==0?0:(tmax-tmin)/(double)tmax;
                match=(targetSaturation<0.05 || h<=tolerance)
                    && Math.Abs(saturation-targetSaturation)<=tolerance && Math.Abs(max-tmax)<=255*tolerance;
            }
            bytes[i]=bytes[i+1]=bytes[i+2]=(byte)(match?0:255);
        }
        Write(image,bytes);
    }

    // A rectangular 3x3 erosion, as offered by MORT. Optional: can join nearby glyphs.
    public static void Erode(Bitmap image)
    {
        var source=Capture.Pixels(image);var output=(byte[])source.Clone();
        for(int y=0;y<image.Height;y++)for(int x=0;x<image.Width;x++)for(int c=0;c<3;c++)
        {
            byte minimum=255;
            for(int dy=-1;dy<=1;dy++)for(int dx=-1;dx<=1;dx++)
            {
                int sx=Math.Clamp(x+dx,0,image.Width-1),sy=Math.Clamp(y+dy,0,image.Height-1);
                minimum=Math.Min(minimum,source[(sy*image.Width+sx)*4+c]);
            }
            output[(y*image.Width+x)*4+c]=minimum;
        }
        Write(image,output);
    }

    public static OcrBlock Colors(Bitmap image, OcrBlock block)
    {
        var counts=new Dictionary<int,int>();
        int left=Math.Clamp((int)block.X,0,image.Width-1),top=Math.Clamp((int)block.Y,0,image.Height-1);
        int right=Math.Clamp((int)Math.Ceiling(block.X+block.Width),left+1,image.Width);
        int bottom=Math.Clamp((int)Math.Ceiling(block.Y+block.Height),top+1,image.Height);
        for(int y=top;y<bottom;y+=Math.Max(1,(bottom-top)/24))for(int x=left;x<right;x+=Math.Max(1,(right-left)/24))
        {
            var c=image.GetPixel(x,y);int key=((c.R/16)<<8)|((c.G/16)<<4)|(c.B/16);
            counts[key]=counts.GetValueOrDefault(key)+1;
        }
        var keyColor=counts.MaxBy(p=>p.Value).Key;
        var bg=Color.FromArgb(((keyColor>>8)&15)*17,((keyColor>>4)&15)*17,(keyColor&15)*17);
        var foreground=bg.R*0.299+bg.G*0.587+bg.B*0.114>145?"#000000":"#ffffff";
        return block with { Background=$"#{bg.R:x2}{bg.G:x2}{bg.B:x2}",Foreground=foreground };
    }
}
