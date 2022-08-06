using AsmResolver.DotNet;
using AsmResolver.DotNet.Serialized;
using DllsGen;

// TODO: cli interface thing
var publicizeMethods = false;
var outputPath = "/home/stack/ledump/QuestModding/qmerge/MergeExample/build/Managed/";
var inputPath = "/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out";

var referencePaths = new[]
{
    // Mono Assemblies
    "/home/stack/ledump/UnityEditor/2019.4.28f1/Editor/Data/MonoBleedingEdge/lib/mono/unityaot/",
    // Unity Assembles
    "/home/stack/ledump/UnityEditor/2019.4.28f1/Editor/Data/PlaybackEngines/AndroidPlayer/Variations/il2cpp/Managed/",
    // If all else fails, use assembles from mono version of game
    "/home/stack/.local/share/Steam/steamapps/common/Beat Saber/Beat Saber_Data/Managed/",
};

if (Directory.Exists(outputPath))
{
    Directory.Delete(outputPath, true);
    Directory.CreateDirectory(outputPath);
}

var inputPaths = Directory.GetFiles(inputPath);
var readingParams = new ModuleReaderParameters(inputPath);

var dummyModule = new ModuleReference("MergePInvokeDummy");

var inputModules = new List<ModuleDefinition>();
var refModules = new List<ModuleDefinition?>();
var refToShimAssembly = new Dictionary<string, AssemblyDefinition>();
foreach (var path in inputPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.FromFile(path, readingParams);
    inputModules.Add(module);

    ModuleDefinition? referenceModule = null;
    foreach (var referencePath in referencePaths)
    {
        var referenceModulePath = referencePath + fileName;
        if (File.Exists(referenceModulePath))
        {
            Console.WriteLine("Using reference at " + referenceModulePath);
            referenceModule = ModuleDefinition.FromFile(referenceModulePath);
            refToShimAssembly.Add(referenceModule.Assembly.Name, module.Assembly);
            break;
        }
    }

    refModules.Add(referenceModule);
}

// wtf
SmallFixes.FixIntPtrInModule(refToShimAssembly["mscorlib"].ManifestModule);

foreach (var (module, referenceModule) in inputModules.Zip(refModules))
{
    var processor = new ModuleProcessor(module, referenceModule, dummyModule, refToShimAssembly);
    processor.Process();

    module.Write(outputPath + module.Name);
}

Console.WriteLine("Done!");