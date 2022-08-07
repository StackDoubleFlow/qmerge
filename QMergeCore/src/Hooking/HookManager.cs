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
            var original = hook.parameterTypes switch
            {
                null => hook.type.GetMethod(hook.methodName, AllLookupFlags),
                _ => hook.type.GetMethod(hook.methodName, AllLookupFlags, null, hook.parameterTypes, null)
            };
            if (original == null)
                throw new Exception("could not find method to hook");

            var methods = type.GetMethods(AllLookupFlags);
            MethodInfo? prefix = null;
            MethodInfo? postfix = null;
            foreach (var methodInfo in methods)
            {
                switch (methodInfo.Name)
                {
                    case "Prefix":
                    {
                        if (prefix != null)
                            Debug.LogWarning($"Found multiple postfixes in hook {type}");
                        prefix = methodInfo;
                        break;
                    }
                    case "Postfix":
                    {
                        if (postfix != null)
                            Debug.LogWarning($"Found multiple prefixes in hook {type}");
                        postfix = methodInfo;
                        break;
                    }
                }
            }

            if (postfix == null && prefix == null)
            {
                Debug.Log("Could not find prefix or postfix in hook {type}");
                return;
            }

            CreateHookNative(original, prefix, postfix);
        }

        private static void CreateHookNative(MethodInfo original, MethodInfo? prefix, MethodInfo? postfix)
        {
            NativeHelper.NativeStub(original, prefix, postfix);
        }
    }
}