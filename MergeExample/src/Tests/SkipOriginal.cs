using QMerge.Hooking;

namespace MergeExample.Tests
{
    [Hook(typeof(SkipOriginal), "Test")]
    public class SkipOriginal
    {
        private static bool Prefix(ref int __result)
        {
            Plugin.Logger.Debug("Starting SkipOriginal");
            if (__result != 0)
            {
                Plugin.Logger.Debug("__result default value was incorrect");
            }
            __result = 420;
            return false;
        }
        
        private static void Postfix(int __result, bool __runOriginal)
        {
            if (__result == 420 && !__runOriginal)
            {
                Plugin.Logger.Debug("SkipOriginal passed");
            }
            else if (__result == 69)
            {
                Plugin.Logger.Debug("SkipOriginal failed with original result");
            }
            else
            {
                Plugin.Logger.Debug($"SkipOriginal catastrophically (got {__result})");
            }
        }

        private static int Test()
        {
            return 69;
        }

        public static void RunTest()
        {
            Test();
        }
    }
}