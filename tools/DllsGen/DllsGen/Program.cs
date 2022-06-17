using AsmResolver.DotNet;
using AsmResolver.DotNet.Serialized;
using AsmResolver.PE.DotNet.Metadata.Tables.Rows;

// TODO: cli interface thing
var publicizeMethods = false;
var managedPath = "/home/stack/ledump/QuestModding/qmerge/example_mod/build/Managed/";
var inputPath = "/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out";

if (Directory.Exists(managedPath))
{
    Directory.Delete(managedPath, true);
    Directory.CreateDirectory(managedPath);
}

var dummyPaths = Directory.GetFiles(inputPath);

var readingParams = new ModuleReaderParameters(inputPath);

var dummyModule = new ModuleReference("MergePInvokeDummy");

static void ProcessType(TypeDefinition type, ModuleReference dummyModule, bool publicizeMethods)
{
    foreach (var method in type.Methods.Where(method => method.IsPInvokeImpl))
    {
        const ImplementationMapAttributes attributes = ImplementationMapAttributes.NoMangle | ImplementationMapAttributes.CharSetAuto | ImplementationMapAttributes.CallConvCdecl;
        method.ImplementationMap = new ImplementationMap(dummyModule, "MergePInvokeDummy", attributes);
    }
}

foreach (var path in dummyPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.FromFile(path, readingParams);

    var imported = module.DefaultImporter.ImportModule(dummyModule);
    foreach (var type in module.GetAllTypes())
    {
        ProcessType(type, imported, publicizeMethods);
    }
    
    module.Write(managedPath + fileName);
}

Console.WriteLine("Done!");
