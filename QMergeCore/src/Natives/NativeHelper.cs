using System;
using System.Runtime.CompilerServices;

namespace QMerge.Natives
{
    internal class NativeHelper
    {
        // We want this method to inline so the native proxy method is large enough to hook.
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        internal static void NativeStub(params object?[] p)
        {
            throw new Exception("Hit native stub. An internal method was not replaced!");
        }
    }
}