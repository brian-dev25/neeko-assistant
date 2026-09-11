// Windows Graphics Capture interop follows Microsoft's documented HWND/DXGI APIs.
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using Windows.Graphics.Capture;
using Windows.Graphics.DirectX;
using Windows.Graphics.DirectX.Direct3D11;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace Neeko.ScreenOcr;

internal sealed class WindowCapture : IDisposable
{
    [ComImport, Guid("3628E81B-3CAC-4C60-B7F4-23CE0E0C3356"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IItemInterop { nint CreateForWindow(nint window, in Guid iid); nint CreateForMonitor(nint monitor, in Guid iid); }
    [DllImport("combase.dll")] private static extern int WindowsCreateString([MarshalAs(UnmanagedType.LPWStr)] string value, int length, out nint str);
    [DllImport("combase.dll")] private static extern int WindowsDeleteString(nint str);
    [DllImport("combase.dll")] private static extern int RoGetActivationFactory(nint name, in Guid iid, out nint factory);
    [DllImport("d3d11.dll")] private static extern int D3D11CreateDevice(nint adapter, uint type, nint software, uint flags, nint levels, uint count, uint sdk, out nint device, out uint level, out nint context);
    [DllImport("d3d11.dll")] private static extern int CreateDirect3D11DeviceFromDXGIDevice(nint device, out nint graphics);
    private readonly IDirect3DDevice device;
    private readonly Direct3D11CaptureFramePool pool;
    private readonly GraphicsCaptureSession session;
    private readonly object gate = new();
    private TaskCompletionSource<Bitmap>? pending;
    private Windows.Graphics.SizeInt32 size;
    private bool disposed;
    public WindowCapture(nint window)
    {
        if(!GraphicsCaptureSession.IsSupported()) throw new InvalidOperationException("Windows Graphics Capture no está disponible.");
        const string className="Windows.Graphics.Capture.GraphicsCaptureItem";
        Marshal.ThrowExceptionForHR(WindowsCreateString(className,className.Length,out var name));
        GraphicsCaptureItem item;
        try
        {
            Marshal.ThrowExceptionForHR(RoGetActivationFactory(name,typeof(IItemInterop).GUID,out var factory));
            try
            {
                var interop=(IItemInterop)Marshal.GetObjectForIUnknown(factory);
                var pointer=interop.CreateForWindow(window,new Guid("79C3F95B-31F7-4EC2-A464-632EF5D30760"));
                try { item=WinRT.MarshalInterface<GraphicsCaptureItem>.FromAbi(pointer); }
                finally {Marshal.Release(pointer);Marshal.ReleaseComObject(interop);}
            }
            finally {Marshal.Release(factory);}
        }
        finally {WindowsDeleteString(name);}
        Marshal.ThrowExceptionForHR(D3D11CreateDevice(0,1,0,0x20,0,0,7,out var d3d,out _,out var context));
        try
        {
            var iid=new Guid("54EC77FA-1377-44E6-8C32-88FD5F44C84C");
            Marshal.ThrowExceptionForHR(Marshal.QueryInterface(d3d,in iid,out var dxgi));
            try
            {
                Marshal.ThrowExceptionForHR(CreateDirect3D11DeviceFromDXGIDevice(dxgi,out var pointer));
                try {device=WinRT.MarshalInterface<IDirect3DDevice>.FromAbi(pointer);} finally {Marshal.Release(pointer);}
            }
            finally {Marshal.Release(dxgi);}
        }
        finally {Marshal.Release(context);Marshal.Release(d3d);}
        size=item.Size;
        pool=Direct3D11CaptureFramePool.CreateFreeThreaded(device,DirectXPixelFormat.B8G8R8A8UIntNormalized,2,size);
        session=pool.CreateCaptureSession(item);session.IsCursorCaptureEnabled=false;
        pool.FrameArrived+=Arrived;session.StartCapture();
    }
    private async void Arrived(Direct3D11CaptureFramePool sender, object args)
    {
        TaskCompletionSource<Bitmap>? request=null;
        try
        {
            using var frame=sender.TryGetNextFrame();if(frame==null)return;
            lock(gate) { if(disposed)return;request=pending;pending=null; }
            if(frame.ContentSize.Width!=size.Width || frame.ContentSize.Height!=size.Height)
            {
                size=frame.ContentSize;
                if(size.Width<20 || size.Height<20 || (long)size.Width*size.Height>33_000_000) throw new InvalidOperationException("Tamaño de ventana no compatible.");
                sender.Recreate(device,DirectXPixelFormat.B8G8R8A8UIntNormalized,2,size);
                request?.TrySetException(new InvalidOperationException("La ventana cambió de tamaño. Volvé a iniciar la captura."));return;
            }
            if(request==null)return;
            using var software=await SoftwareBitmap.CreateCopyFromSurfaceAsync(frame.Surface);
            using var converted=SoftwareBitmap.Convert(software,BitmapPixelFormat.Bgra8,BitmapAlphaMode.Ignore);
            var bytes=new byte[converted.PixelWidth*converted.PixelHeight*4];
            var buffer=new Windows.Storage.Streams.Buffer((uint)bytes.Length);converted.CopyToBuffer(buffer);
            using(var reader=DataReader.FromBuffer(buffer))reader.ReadBytes(bytes);
            var image=new Bitmap(converted.PixelWidth,converted.PixelHeight,PixelFormat.Format32bppArgb);
            var data=image.LockBits(new Rectangle(0,0,image.Width,image.Height),ImageLockMode.WriteOnly,PixelFormat.Format32bppArgb);
            try {for(int y=0;y<image.Height;y++)Marshal.Copy(bytes,y*image.Width*4,data.Scan0+y*data.Stride,image.Width*4);}
            finally {image.UnlockBits(data);}
            if(!request.TrySetResult(image))image.Dispose();
        }
        catch(Exception error) {request?.TrySetException(error);}
    }
    public async Task<Bitmap> Read()
    {
        var request=new TaskCompletionSource<Bitmap>(TaskCreationOptions.RunContinuationsAsynchronously);
        lock(gate) {ObjectDisposedException.ThrowIf(disposed,this);pending=request;}
        try {return await request.Task.WaitAsync(TimeSpan.FromSeconds(5));}
        catch {request.TrySetCanceled();throw;}
        finally {lock(gate)if(pending==request)pending=null;}
    }
    public void Dispose()
    {
        lock(gate){if(disposed)return;disposed=true;pending?.TrySetCanceled();pending=null;}
        pool.FrameArrived-=Arrived;session.Dispose();pool.Dispose();device.Dispose();
    }
}
