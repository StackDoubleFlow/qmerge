using QMerge.Hooking;
using UnityEngine;

namespace MergeExample
{
    public class Tests
    {
        public static void RunTests()
        {
            HFATest.RunTest();
        }
    }
    
    [Hook(typeof(HFATest), "Test")]
    class HFATest
    {
        private static HFA A = new HFA(1, 2, 3, 4);
        private static HFA B = new HFA(5, 6, 7, 8);
        private static HFA C = new HFA(8, 10, 11, 12);
        private static HFA D = new HFA(13, 14, 15, 16);
        
        private struct HFA
        {
            double a;
            double b;
            double c;
            double d;

            public HFA(double a, double b, double c, double d)
            {
                this.a = a;
                this.b = b;
                this.c = c;
                this.d = d;
            }
        }

        private static void Postfix(HFA d, HFA c, HFA b, HFA a)
        {
            if (a.Equals(A) && b.Equals(B) && c.Equals(C) && d.Equals(D))
            {
                Debug.Log("HFATest passed");
            }
            else
            {
                Debug.Log("HFATest failed");
            }
        }

        private static void Test(HFA a, HFA b, HFA c, HFA d)
        {
            Debug.Log("Starting HFATest with " + a + b + c + d);
        }

        public static void RunTest()
        {
            Test(A, B, C, D);
        }
    }
}