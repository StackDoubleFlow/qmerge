namespace MergeExample
{
    public class Plugin
    {
        // static T TestGenerics<T>(T a, T b) {
        //     b = a;
        //     return a;
        // }

        public int TestThings(int a, int b) {
            return a * b;
        }

        public string StringTest() {
            return "Hello World";
        }

        // public void Dummy() {
        //     TestGenerics<int>(1, 2);
        //     TestGenerics<double>(1.0, 2.0);
        // }
    }
}