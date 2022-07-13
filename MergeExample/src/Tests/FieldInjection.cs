using QMerge;
using QMerge.Hooking;

namespace MergeExample.Tests
{
    [Hook(typeof(Struct), "Test")]
    public class FieldInjection
    {
        private struct Struct
        {
            public long a;
            public int b;
            public short c;
            public byte d;

            public string Debug() => $"{a} {b} {c} {d}";
            
            public void Test()
            {
                Plugin.Logger.Debug("Running FieldInjection with " + Debug());
            }
        }

        private static void Postfix(byte ___d, short ___c, int ___b, long ___a, ref Struct __instance)
        {
            Plugin.Logger.Debug("in postfix");
            var s = new Struct
            {
                a = ___a,
                b = ___b,
                c = ___c,
                d = ___d
            };
            if (__instance.Equals(s))
            {
                Plugin.Logger.Debug("FieldInjection passed");
            }
            else
            {
                Plugin.Logger.Debug("FieldInjection failed");
                Plugin.Logger.Debug("Expected: " + __instance.Debug());
                Plugin.Logger.Debug("Found: " + s.Debug());
            }
        }

        public static void RunTest()
        {
            Plugin.Logger.Debug("Starting FieldInjection");
            var s = new Struct
            {
                a = 5768,
                b = 3625,
                c = 4749,
                d = 194
            };
            s.Test();
        }
    }
}