using UnityEngine;
using System.Reflection;
using MergeExample;
using MergeExample.Tests;
using QMerge.Hooking;
using QMLogger = QMerge.Logging.Logger;

public class Plugin
{
    internal static readonly QMLogger Logger = new QMLogger("MergeExample");
    
    public static void Init()
    {
        Logger.Info("Initializing MergeExample");
        var hookManager = new HookManager();
        hookManager.HookAll(Assembly.GetExecutingAssembly());
        
        Logger.Info("Running tests");
        Tests.RunTests();
    }
}