using System;
using System.Reflection;
using QMerge.Natives;
using UnityEngine;

namespace QMerge.Hooking
{
    public class HookManager
    {
        private const BindingFlags AllLookupFlags = BindingFlags.Public
                                                    | BindingFlags.NonPublic
                                                    | BindingFlags.Instance
                                                    | BindingFlags.Static
                                                    | BindingFlags.GetField
                                                    | BindingFlags.SetField
                                                    | BindingFlags.GetProperty
                                                    | BindingFlags.SetProperty;
        
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
            foreach (var customAttribute in type.GetCustomAttributes(false))
            {
                if (customAttribute is Hook hook)
                {
                    CreateHook(hook, type);
                }
            }
        }

        private void CreateHook(Hook hook, Type type)
        {
            var original = hook.type.GetMethod(hook.methodName, AllLookupFlags);
            if (original == null)
                throw new Exception("could not find method to hook");

            var methods = type.GetMethods(AllLookupFlags);
            foreach (var methodInfo in methods)
            {
                if (methodInfo.Name == "Postfix")
                {
                    CreatePostfixHook(original, methodInfo);
                } 
                else if (methodInfo.Name == "Prefix")
                {
                    CreatePrefixHook(original, methodInfo);
                }
            }
        }

        private static void CreatePostfixHook(MethodInfo original, MethodInfo hook)
        {
            NativeHelper.NativeStub(original, hook);
        }
        
        private static void CreatePrefixHook(MethodInfo original, MethodInfo hook)
        {
            NativeHelper.NativeStub(original, hook);
        }
    }
}