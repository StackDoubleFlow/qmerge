// From: https://docs.microsoft.com/en-us/dotnet/api/system.collections.generic.list-1?view=net-6.0#examples
using UnityEngine;
using System;
using System.Collections.Generic;
using MergeExample.Hooks;

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
    }
}