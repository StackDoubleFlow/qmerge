using Mono.Cecil;

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

    foreach (var method in type.Methods.Where(method => method.HasPInvokeInfo))
    {
        Console.WriteLine("Writing dummy PInvokeInfo for {0}", method.FullName);
        const PInvokeAttributes attributes = PInvokeAttributes.NoMangle | PInvokeAttributes.CharSetAuto | PInvokeAttributes.CallConvCdecl;
        method.PInvokeInfo = new PInvokeInfo(attributes, "MergePInvokeDummy", dummyModule);
    }
}

foreach (var path in dummyPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.ReadModule(path, readingParams);

    module.ModuleReferences.Add(dummyModule);
    foreach (var type in module.Types)
    {
        ProcessType(type, dummyModule);
    }
    
    module.Write(managedPath + fileName);
}

Console.WriteLine("Done!");
