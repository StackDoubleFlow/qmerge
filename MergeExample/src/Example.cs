using UnityEngine;
using System.Reflection;
using QMerge.Hooking;

public class Plugin
{
    public static void Init()
    {
        Debug.Log("Initializing MergeExample");
        var hookManager = new HookManager();
        hookManager.HookAll(Assembly.GetExecutingAssembly());
    }
}