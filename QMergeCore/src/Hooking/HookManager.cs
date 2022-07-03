using System;
using System.Reflection;
using QMerge.Natives;

namespace QMerge.Hooking
{
    public class HookManager
    {
        public void HookAll(Assembly assembly)
        {
            var types = assembly.GetTypes();
            foreach (var type in types)
            {
                ProcessType(type);
            }
        }

        private void ProcessType(Type type)
        {
            foreach (var customAttribute in type.GetCustomAttributes())
            {
                if (customAttribute is Hook hook)
                {
                    CreateHook(hook, type);
                }
            }
        }

        private void CreateHook(Hook hook, Type type)
        {
            var methods = type.GetMethods();
            foreach (var methodInfo in methods)
            {
                if (methodInfo.Name == "Postfix")
                {
                    var original = hook.type.GetMethod(hook.methodName);
                    CreatePostfixHook(original, methodInfo);
                } 
                else if (methodInfo.Name == "Prefix")
                {
                    var original = hook.type.GetMethod(hook.methodName);
                    CreatePrefixHook(original, methodInfo);
                }
            }
        }

        private static void CreatePostfixHook(MethodInfo original, MethodInfo hook)
        {
            NativeHelper.NativeStub();
        }
        
        private static void CreatePrefixHook(MethodInfo original, MethodInfo hook)
        {
            NativeHelper.NativeStub();
        }
    }
}