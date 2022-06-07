using DllsGen;
using Mono.Cecil;
using Mono.Cecil.Rocks;

var managedPath = "/home/stack/ledump/QuestModding/qmerge/example_mod/build/Managed/";
if (Directory.Exists(managedPath))
{
    Directory.Delete(managedPath, true);
    Directory.CreateDirectory(managedPath);
}

var dummyPaths = Directory.GetFiles("/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out");

var assemblyResolver = new DefaultAssemblyResolver();
assemblyResolver.AddSearchDirectory("/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out");
var readingParams = new ReaderParameters
{
    AssemblyResolver = assemblyResolver
};

var dummyModule = new ModuleReference("MergePInvokeDummy");

static void ProcessType(TypeDefinition type, ModuleReference dummyModule)
{
    foreach (var t in type.NestedTypes)
    {
        ProcessType(t, dummyModule);
    }
    
    // VTableFixer.FixType(type);
    foreach (var method in type.Methods.Where(method => method.HasPInvokeInfo))
    {
        // Console.WriteLine("Writing dummy PInvokeInfo for {0}", method.FullName);
        const PInvokeAttributes attributes = PInvokeAttributes.NoMangle | PInvokeAttributes.CharSetAuto | PInvokeAttributes.CallConvCdecl;
        method.PInvokeInfo = new PInvokeInfo(attributes, "MergePInvokeDummy", dummyModule);
    }
}

static void CheckTypeOverrides(TypeDefinition type, ModuleDefinition module, ModuleDefinition pcModule)
{
    var pcType = pcModule.GetType(type.FullName);
    if (pcType == null) return;
    
    if (type.FullName == "StateBuffer`3/TimestampedStateTable")
    {
        foreach (var interfaceImplementation in type.Interfaces)
        {
            foreach (var methodDefinition in interfaceImplementation.InterfaceType.Resolve().Methods)
            {
                Console.WriteLine($"Interface method: {methodDefinition.FullName}");
            }
        }
        
        foreach (var methodDefinition in type.GetMethods())
        {
            Console.WriteLine(methodDefinition.Name);
            foreach (var methodDefinitionOverride in methodDefinition.Overrides)
            {
                Console.WriteLine($"Overrides: {methodDefinitionOverride.FullName}");
            }
        }
        Console.WriteLine();
        
        foreach (var methodDefinition in pcType.GetMethods())
        {
            Console.WriteLine(methodDefinition.Name);
            foreach (var methodDefinitionOverride in methodDefinition.Overrides)
            {
                Console.WriteLine($"Overrides: {methodDefinitionOverride.FullName}");
            }
        }
    }
    
    foreach (var myIteratorMethod in type.Methods)
    {
        var pcIteratorMethod = pcType.GetMethods().FirstOrDefault(m => m.FullName == myIteratorMethod.FullName);
        if (pcIteratorMethod == null)
        {
            pcIteratorMethod = pcType.GetMethods().FirstOrDefault(m => m.Name == myIteratorMethod.Name);
        }
        if (pcIteratorMethod == null || myIteratorMethod.Name == "Finalize")
        {
            
            // Console.WriteLine($"Could not find {myIteratorMethod.FullName} in pc");
            continue;
        }

        // if (myIteratorMethod.Overrides.Count != pcIteratorMethod.Overrides.Count)
        // {
        //     Console.WriteLine("big uh oh");
        //     Console.WriteLine(myIteratorMethod.FullName);
        //     Console.WriteLine(myIteratorMethod.Overrides.Count);
        //     Console.WriteLine(pcIteratorMethod.Overrides.Count);
        // }
        // foreach (var (myOverride, pcOverride) in myIteratorMethod.Overrides.Zip(pcIteratorMethod.Overrides))
        // {
        //     if (myOverride.FullName != pcOverride.FullName)
        //     {
        //         Console.WriteLine("uh oh");
        //         Console.WriteLine(myIteratorMethod.FullName);
        //         Console.WriteLine(myOverride.FullName);
        //         Console.WriteLine(pcOverride.FullName);
        //     }
        // }
        if (myIteratorMethod.Overrides.Count != pcIteratorMethod.Overrides.Count)
        {
            Console.WriteLine($"I have {myIteratorMethod.Overrides.Count} while pc has {pcIteratorMethod.Overrides.Count} on {myIteratorMethod.FullName}");
            Console.WriteLine("I have:");
            foreach (var methodReference in myIteratorMethod.Overrides)
            {
                Console.WriteLine(methodReference.FullName);
            }
            Console.WriteLine("pc has:");
            foreach (var methodReference in pcIteratorMethod.Overrides)
            {
                Console.WriteLine(methodReference.FullName);
            }
            Console.WriteLine();
            // if (pcIteratorMethod.Overrides.Count == 0)
            // {
            //     myIteratorMethod.Overrides.Clear();
            // }
        }
        // else
        // {
        //     var broken = false;
        //     foreach (var (first, second) in myIteratorMethod.Overrides.Zip(pcIteratorMethod.Overrides))
        //     {
        //         if (first.FullName != second.FullName)
        //         {
        //             broken = true;
        //             Console.WriteLine(myIteratorMethod.Overrides.Count);
        //             Console.WriteLine(first.FullName);
        //             Console.WriteLine(second.FullName);
        //             throw new Exception();
        //         }
        //     }

            // if (!broken) continue;
        // }
        
        // myIteratorMethod.Overrides.Clear();
        // foreach (var pcOverride in pcIteratorMethod.Overrides)
        // {
        //     var foundDeclType = module.TryGetTypeReference(pcOverride.DeclaringType.FullName, out var overrideDeclType);
        //     if (overrideDeclType == null)
        //     {
        //         Console.WriteLine(pcOverride.DeclaringType.FullName);
        //     }
        //     var overrideMethod = overrideDeclType.Resolve().Methods.FirstOrDefault(method => method.FullName == pcOverride.FullName);
        //     if (overrideMethod == null)
        //     {
        //         Console.WriteLine(pcOverride.FullName);
        //     }
        //
        //     Console.WriteLine($"Importing reference {overrideMethod.FullName}");
        //     module.ImportReference(overrideMethod);
        //     myIteratorMethod.Overrides.Add(overrideMethod);
        // }
    }
}

static void TestThing(ModuleDefinition module, string name)
{
    var pcModule = ModuleDefinition.ReadModule("/home/stack/.local/share/Steam/steamapps/common/Beat Saber/Beat Saber_Data/Managed/" + name);
    var myIterator = module.GetType("System.Linq.Enumerable/Iterator`1");
    // CheckTypeOverrides(myIterator, module, pcModule);
    foreach (var typeDefinition in module.Types)
    {
        foreach (var nestedType in typeDefinition.NestedTypes)
        {
            CheckTypeOverrides(nestedType, module, pcModule);
        }

        CheckTypeOverrides(typeDefinition, module, pcModule);
    }
}

foreach (var path in dummyPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.ReadModule(path, readingParams);

    // if (fileName == "System.Core.dll")
    // {
        TestThing(module, fileName);
    // }

    module.ModuleReferences.Add(dummyModule);
    foreach (var type in module.Types)
    {
        ProcessType(type, dummyModule);
    }
    
    module.Write(managedPath + fileName);
}

Console.WriteLine("Done!");
