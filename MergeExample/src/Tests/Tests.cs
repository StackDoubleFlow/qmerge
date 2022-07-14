namespace MergeExample.Tests
{
    public class Tests
    {
        public static void RunTests()
        {
            HFASingle.RunTest();
            HFADouble.RunTest();
            FieldInjection.RunTest();
            SkipOriginal.RunTest();
        }
    }
}