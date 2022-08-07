using System;

namespace QMerge.Hooking
{
    [AttributeUsage(AttributeTargets.Class)]
    public class Hook : Attribute
    {
        public Type type;
        public string methodName;
        public Type[]? parameterTypes;

        public Hook(Type type, string methodName, Type[]? parameterTypes = null)
        {
            this.type = type;
            this.methodName = methodName;
            this.parameterTypes = parameterTypes;
        }
    }
}