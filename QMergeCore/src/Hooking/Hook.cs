using System;

namespace QMerge.Hooking
{
    [AttributeUsage(AttributeTargets.Class)]
    public class Hook : Attribute
    {
        public Type type;
        public string methodName;

        public Hook(Type type, string methodName)
        {
            this.type = type;
            this.methodName = methodName;
        }
    }

    struct Vector3
    {
        private float x;
        private float y;
        private float z;
        
        public Vector3(float x, float y, float z)
        {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        public void SetX(float x)
        {
            this.x = x;
        }
        
        public static Vector3 operator +(Vector3 a, Vector3 b)
        {
            return new Vector3(a.x + b.x, a.y + b.y, a.z + b.z);
        }
    }
}