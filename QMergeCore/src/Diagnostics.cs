using QMerge.Natives;

namespace QMerge
{
    public class Diagnostics
    {
        public static void Crash()
        {
            NativeHelper.NativeStub();
        }
    }
}