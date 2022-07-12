using QMerge.Hooking;
using UnityEngine;

namespace MergeExample.Tests
{
    [Hook(typeof(HFASingle), "Test")]
    class HFASingle
    {
        private static HFA A = new HFA(1, 2, 3, 4);
        private static HFA B = new HFA(5, 6, 7, 8);
        private static HFA C = new HFA(8, 10, 11, 12);
        private static HFA D = new HFA(13, 14, 15, 16);
        
        private struct HFA
        {
            float a;
            float b;
            float c;
            float d;

            public HFA(float a, float b, float c, float d)
            {
                this.a = a;
                this.b = b;
                this.c = c;
                this.d = d;
            }

            public void Debug()
            {
                UnityEngine.Debug.Log($"{a} {b} {c} {d}");
            }
        }

        private static void Postfix(HFA d, HFA c, HFA b, HFA a)
        {
            if (a.Equals(A) && b.Equals(B) && c.Equals(C) && d.Equals(D))
            {
                Debug.Log("HFASingle passed");
            }
            else
            {
                Debug.Log("HFASingle failed");
                Debug.Log("Found:");
                a.Debug();
                b.Debug();
                c.Debug();
                d.Debug();
                Debug.Log("Expected");
                A.Debug();
                B.Debug();
                C.Debug();
                D.Debug();
            }
        }

        private static void Test(HFA a, HFA b, HFA c, HFA d)
        {
            Debug.Log("Starting HFASingle" + a + b + c + d);
        }

        public static void RunTest()
        {
            Test(A, B, C, D);
        }
    }
}