using UnityEngine;
using System;
using System.Reflection;
using MergeExample;
using MergeExample.Hooks;
using QMerge.Hooking;
using TMPro;
using UnityEngine.SceneManagement;

public class Plugin
{
    public static void Init()
    {
        Debug.Log("Initializing MergeExample");
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