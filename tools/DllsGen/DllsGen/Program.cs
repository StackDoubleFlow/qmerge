using Mono.Cecil;

// TODO: cli interface thing
var publicizeMethods = true;
var managedPath = "/home/stack/ledump/QuestModding/qmerge/example_mod/build/Managed/";
var inputPath = "/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out";

if (Directory.Exists(managedPath))
{
    Directory.Delete(managedPath, true);
    Directory.CreateDirectory(managedPath);
}

var dummyPaths = Directory.GetFiles(inputPath);

var assemblyResolver = new DefaultAssemblyResolver();
assemblyResolver.AddSearchDirectory(inputPath);
var readingParams = new ReaderParameters
{
    AssemblyResolver = assemblyResolver
};

var dummyModule = new ModuleReference("MergePInvokeDummy");

static void ProcessType(TypeDefinition type, ModuleReference dummyModule, bool publicizeMethods)
{
    foreach (var t in type.NestedTypes)
    {
        ProcessType(t, dummyModule, publicizeMethods);
    }
    
    foreach (var method in type.Methods.Where(method => method.HasPInvokeInfo))
    {
        // Console.WriteLine("Writing dummy PInvokeInfo for {0}", method.FullName);
        const PInvokeAttributes attributes = PInvokeAttributes.NoMangle | PInvokeAttributes.CharSetAuto | PInvokeAttributes.CallConvCdecl;
        method.PInvokeInfo = new PInvokeInfo(attributes, "MergePInvokeDummy", dummyModule);
        if (publicizeMethods)
        {
            method.IsPublic = true;
        }
    }
}

foreach (var path in dummyPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.ReadModule(path, readingParams);

    module.ModuleReferences.Add(dummyModule);
    foreach (var type in module.Types)
    {
        ProcessType(type, dummyModule, publicizeMethods);
    }
    
    module.Write(managedPath + fileName);
}

Console.WriteLine("Done!");
