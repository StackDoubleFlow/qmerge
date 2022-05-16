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

        public string StringTest() {
            return "Hello World";
        }

        public void Dummy(TestStruct testStruct) {
            TestGenerics<int>(1, 2);
            TestGenerics2<int>(1, 2);
            TestGenerics<double>(1.0, 2.0);
        }
    }

    public struct TestStruct {
        double a;
        double b;
    }
}
