using UnityEngine;
using System;
using System.Reflection;
using MergeExample.Hooks;
using QMerge.Hooking;

public class Plugin
{
    public static void Init()
    {
        Debug.Log("Hello world");
        Attribute[] attrs = Attribute.GetCustomAttributes(typeof(MainMenuViewControllerDidActivate), false);  // Reflection.  
  
        // Displaying output.  
        foreach (Attribute attr in attrs)  
        {
            Debug.Log(attr.GetType().FullName);
        }

        var hookManager = new HookManager();
        hookManager.HookAll(Assembly.GetExecutingAssembly());
    }
}