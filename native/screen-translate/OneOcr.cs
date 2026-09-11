// Adapted from MORT/OcrApi/OneOcr (MIT), ea9afbcc9da1cf1ecc3d099c397d87d9b2970f5a.
// Copyright (c) 2024 몽키해드. See LICENSE-MORT.txt.
// Neeko loads the user's installed Microsoft components in place; none are redistributed.
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Reflection;

namespace Neeko.ScreenOcr;

internal sealed class OneOcr : IDisposable
{
    [StructLayout(LayoutKind.Sequential)] private struct Img { public int Type,Columns,Rows,Unknown; public long Stride; public nint Data; }
    [StructLayout(LayoutKind.Sequential)] private struct Box { public float X1,Y1,X2,Y2,X3,Y3,X4,Y4; }
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] private static extern nint LoadLibraryEx(string path,nint file,uint flags);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long CreateOcrInitOptions(out long options);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long OcrInitOptionsSetUseModelDelayLoad(long options,byte enabled);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long CreateOcrPipeline([MarshalAs(UnmanagedType.LPUTF8Str)]string path,[MarshalAs(UnmanagedType.LPUTF8Str)]string key,long options,out long pipeline);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long CreateOcrProcessOptions(out long options);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long OcrProcessOptionsSetMaxRecognitionLineCount(long options,long count);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long OcrProcessOptionsSetRunBackendModelOnCPU(long options,long enabled);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long RunOcrPipeline(long pipeline,ref Img image,long options,out long result);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long GetOcrLineCount(long result,out long count);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long GetOcrLine(long result,long index,out long line);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long GetOcrLineContent(long line,out nint text);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern long GetOcrLineBoundingBox(long line,out nint box);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern void ReleaseOcrResult(long result);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern void ReleaseOcrPipeline(long pipeline);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern void ReleaseOcrProcessOptions(long options);
    [DllImport("neeko-oneocr",CallingConvention=CallingConvention.Cdecl)] private static extern void ReleaseOcrInitOptions(long options);
    private static nint module;
    private static string? loadedPath;
    private long pipeline,options;
    public static string? FindPath()
    {
        try
        {
            var manager=new Windows.Management.Deployment.PackageManager();
            foreach(var package in manager.FindPackagesForUser(string.Empty).Where(p=>p.Id.PublisherId=="8wekyb3d8bbwe" && (p.Id.Name=="Microsoft.ScreenSketch" || p.Id.Name=="Microsoft.Windows.Photos")).OrderBy(p=>p.Id.Name=="Microsoft.ScreenSketch" ? 0:1))
                foreach(var folder in new[]{Path.Combine(package.InstalledLocation.Path,"SnippingTool"),package.InstalledLocation.Path})
                    if(File.Exists(Path.Combine(folder,"oneocr.dll")) && File.Exists(Path.Combine(folder,"oneocr.onemodel")))return folder;
        }
        catch { }
        return null;
    }
    public static OcrLanguage Language() => new("auto","OneOCR · detección automática","oneocr",FindPath()!=null,"auto");
    private static void Check(long result) {if(result!=0)throw new InvalidOperationException($"OneOCR devolvió el código {result}. Actualizá Recortes o elegí otro motor.");}
    public OcrBlock[] Recognize(Bitmap source)
    {
        if(pipeline==0)
        {
            var path=FindPath() ?? throw new InvalidOperationException("No se encontró OneOCR en Recortes o Fotos. Actualizá esas aplicaciones o elegí Tesseract.");
            if(module==0)
            {
                // WindowsApps can be readable but deny loading a DLL from its protected directory.
                // Like MORT, prepare a per-user copy from the installed package, never a download.
                var digest=Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(Path.Combine(path,"oneocr.dll"))))[..16];
                var cache=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"neeko-assistant","oneocr",digest);
                Directory.CreateDirectory(cache);
                foreach(var file in new[]{"oneocr.dll","oneocr.onemodel","onnxruntime.dll"})
                {
                    var origin=Path.Combine(path,file);var target=Path.Combine(cache,file);
                    if(!File.Exists(target)||new FileInfo(target).Length!=new FileInfo(origin).Length)File.Copy(origin,target,true);
                }
                path=cache;
                module=LoadLibraryEx(Path.Combine(path,"oneocr.dll"),0,0x1100);
                if(module==0)throw new InvalidOperationException($"No se pudieron cargar los componentes OneOCR instalados (Windows {Marshal.GetLastWin32Error()}).");
                NativeLibrary.SetDllImportResolver(Assembly.GetExecutingAssembly(),(name,_,_)=>name=="neeko-oneocr" ? module : 0);
                loadedPath=path;
            }
            path=loadedPath!;
            Check(CreateOcrInitOptions(out var init));
            try
            {
                Check(OcrInitOptionsSetUseModelDelayLoad(init,0));
                Check(CreateOcrPipeline(Path.Combine(path,"oneocr.onemodel"),"kj)TGtrK>f]b[Piow.gU+nC@s\"\"\"\"\"\"4",init,out pipeline));
                Check(CreateOcrProcessOptions(out options));
                Check(OcrProcessOptionsSetMaxRecognitionLineCount(options,100));
                Check(OcrProcessOptionsSetRunBackendModelOnCPU(options,1));
            }
            finally {ReleaseOcrInitOptions(init);}
        }
        using var bitmap=new Bitmap(source.Width,source.Height,PixelFormat.Format24bppRgb);
        using(var graphics=Graphics.FromImage(bitmap))graphics.DrawImageUnscaled(source,0,0);
        var data=bitmap.LockBits(new Rectangle(0,0,bitmap.Width,bitmap.Height),ImageLockMode.ReadOnly,PixelFormat.Format24bppRgb);
        long result=0;
        try
        {
            var image=new Img{Type=1,Columns=bitmap.Width,Rows=bitmap.Height,Stride=data.Stride,Data=data.Scan0};
            Check(RunOcrPipeline(pipeline,ref image,options,out result));Check(GetOcrLineCount(result,out var count));
            var blocks=new List<OcrBlock>();
            for(long i=0;i<Math.Min(count,100);i++)
            {
                Check(GetOcrLine(result,i,out var line));Check(GetOcrLineContent(line,out var pointer));
                var text=Marshal.PtrToStringUTF8(pointer) ?? "";
                Check(GetOcrLineBoundingBox(line,out pointer));var box=Marshal.PtrToStructure<Box>(pointer);
                var x=Math.Max(0,new[]{box.X1,box.X2,box.X3,box.X4}.Min());var y=Math.Max(0,new[]{box.Y1,box.Y2,box.Y3,box.Y4}.Min());
                var right=Math.Min(bitmap.Width,new[]{box.X1,box.X2,box.X3,box.X4}.Max());var bottom=Math.Min(bitmap.Height,new[]{box.Y1,box.Y2,box.Y3,box.Y4}.Max());
                if(!string.IsNullOrWhiteSpace(text)&&right>x&&bottom>y)blocks.Add(new(text,x,y,right-x,bottom-y));
            }
            return blocks.ToArray();
        }
        finally {if(result!=0)ReleaseOcrResult(result);bitmap.UnlockBits(data);}
    }
    public void Dispose(){if(options!=0){ReleaseOcrProcessOptions(options);options=0;}if(pipeline!=0){ReleaseOcrPipeline(pipeline);pipeline=0;}}
}
