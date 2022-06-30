using System;

namespace QMerge.Hooks
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
}