using UnityEngine;

namespace MergeExample
{
    public class Plugin
    {
        static T TestGenerics<T>(T a, T b) {
            System.Type type = typeof(T);
            b = a;
            int c = new Plugin().TestThings(1, 1);
            return a;
        }

        static T TestGenerics2<T>(T a, T b) {
            System.Type type = typeof(T);
            b = a;
            return a;
        }

        public int TestThings(int a, int b) {
            return a * b;
        }

        public static string StringTest() {
            return "Hello World";
        }

        public static void Dummy(TestStruct testStruct) {
            Debug.Log(TestGenerics<int>(1, 2));
            Debug.Log(TestGenerics2<int>(1, 2));
            Debug.Log(TestGenerics<double>(1.0, 2.0));
        }

        public static void Load() {
            Debug.Log("Hello from c#!");
            Dummy(new TestStruct());
            Debug.Log(StringTest());
        }
    }

    public struct TestStruct {
        double a;
        double b;
    }
}
