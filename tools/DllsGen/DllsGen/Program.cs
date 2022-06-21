using AsmResolver.DotNet;
using AsmResolver.DotNet.Code.Cil;
using AsmResolver.DotNet.Collections;
using AsmResolver.DotNet.Serialized;
using AsmResolver.DotNet.Signatures;
using AsmResolver.DotNet.Signatures.Types;
using AsmResolver.PE.DotNet.Cil;
using AsmResolver.PE.DotNet.Metadata.Tables.Rows;
using DllsGen;

// TODO: cli interface thing
var publicizeMethods = false;
var managedPath = "/home/stack/ledump/QuestModding/qmerge/example_mod/build/Managed/";
var inputPath = "/home/stack/ledump/QuestModding/qmerge/test_analysis/cpp2il_out";

var referencePaths = new string[]
{
    "/home/stack/ledump/UnityEditor/2019.4.28f1/Editor/Data/MonoBleedingEdge/lib/mono/unityaot/",
    "/home/stack/.local/share/Steam/steamapps/common/Beat Saber/Beat Saber_Data/Managed/",
};

if (Directory.Exists(managedPath))
{
    Directory.Delete(managedPath, true);
    Directory.CreateDirectory(managedPath);
}

var inputPaths = Directory.GetFiles(inputPath);

var readingParams = new ModuleReaderParameters(inputPath);

var dummyModule = new ModuleReference("MergePInvokeDummy");

static void FixIntPtrInModule(ModuleDefinition module)
{
    foreach (var type in module.TopLevelTypes)
    {
        foreach (var methodDefinition in type.Methods)
        {
            var sig = methodDefinition.Signature;
            if (sig.ReturnType.ElementType == ElementType.ValueType && sig.ReturnType.Namespace == "System" && sig.ReturnType.Name == "IntPtr")
            {
                Console.WriteLine("Fixing IntPtr in " + methodDefinition.Module.FilePath);
                Console.WriteLine("For method " + methodDefinition);
                sig.ReturnType = module.CorLibTypeFactory.IntPtr;
            }

            for (var i = 0; i < sig.ParameterTypes.Count; i++)
            {
                var paramType = sig.ParameterTypes[i];
                if (paramType.IsValueType && paramType.Namespace == "System" && paramType.Name == "IntPtr")
                {
                    sig.ParameterTypes[i] = module.CorLibTypeFactory.IntPtr;
                }
                if (paramType.IsValueType && paramType.Namespace == "System" && paramType.Name == "UIntPtr")
                {
                    sig.ParameterTypes[i] = module.CorLibTypeFactory.UIntPtr;
                }
            }
        }
    }
}

static TypeDefinition? FindTypeInModule(TypeDefinition type, ModuleDefinition module)
{
    IResolutionScope scope = module;
    if (type.DeclaringType != null)
    {
        var declType = FindTypeInModule(type.DeclaringType, module);
        if (declType == null)
            return null;
        scope = declType.ToTypeReference();
    }
    
    return new TypeReference(module, scope, type.Namespace, type.Name).Resolve();
}

static void ProcessType(TypeDefinition type, ModuleDefinition module, ModuleReference dummyModule, ModuleDefinition? referenceModule, ReferenceConverter converter, SignatureComparer comparer)
{
    if (type.Namespace == "Cpp2IlInjected")
        return;
    var referenceType = FindTypeInModule(type, referenceModule);
    if (referenceType == null)
        Console.WriteLine("Could not find reference type for " + type);

    foreach (var method in type.Methods)
    {
        if (method.IsPInvokeImpl)
        {
            const ImplementationMapAttributes attributes = ImplementationMapAttributes.NoMangle | ImplementationMapAttributes.CharSetAuto | ImplementationMapAttributes.CallConvCdecl;
            method.ImplementationMap = new ImplementationMap(dummyModule, "MergePInvokeDummy", attributes);
        }

        if (type.GenericParameters.Count > 0 || method.GenericParameters.Count > 0)
        {
            var referenceMethod = (MethodDefinition?) referenceType?.CreateMemberReference(method.Name, method.Signature.ImportWith(referenceModule.DefaultImporter)).Resolve();
            if (referenceType != null && referenceMethod == null)
            {
                foreach (var referenceTypeMethod in referenceType.Methods)
                {
                    if (referenceTypeMethod.Name == method.Name)
                    {
                        try
                        {
                            var convertedSig = converter.Convert(referenceTypeMethod.Signature);
                            if (comparer.Equals(convertedSig, method.Signature))
                            {
                                referenceMethod = referenceTypeMethod;
                                break;
                            }
                        }
                        catch (Exception _)
                        {
                            // Console.WriteLine("failed to find reference for " + method);
                        }
                    }
                }
            }
            if (referenceMethod?.CilMethodBody == null) continue;
        
            var body = new CilMethodBody(method);
            var referenceBody = referenceMethod.CilMethodBody;

            body.MaxStack = referenceBody.MaxStack;
            body.ComputeMaxStackOnBuild = false;
            body.InitializeLocals = referenceBody.InitializeLocals;
            
            foreach (var localVariable in referenceBody.LocalVariables)
            {
                body.LocalVariables.Add(new CilLocalVariable(converter.Convert(localVariable.VariableType).ImportWith(module.DefaultImporter)));
            }
            
            foreach (var exceptionHandler in referenceBody.ExceptionHandlers)
            {
                ICilLabel? filterStart;
                if (exceptionHandler.FilterStart is { } l)
                {
                    filterStart = l;
                }
                else
                {
                    filterStart = null;
                }
                body.ExceptionHandlers.Add(new CilExceptionHandler
                {
                    HandlerType = exceptionHandler.HandlerType,
                    TryStart = new CilOffsetLabel(exceptionHandler.TryStart.Offset),
                    TryEnd = new CilOffsetLabel(exceptionHandler.TryEnd.Offset),
                    HandlerStart = new CilOffsetLabel(exceptionHandler.HandlerStart.Offset),
                    HandlerEnd = new CilOffsetLabel(exceptionHandler.HandlerEnd.Offset),
                    FilterStart = filterStart,
                    ExceptionType = exceptionHandler.ExceptionType != null ? converter.Convert(exceptionHandler.ExceptionType).ImportWith(module.DefaultImporter) : null,
                });
            }
            
            try
            {
                foreach (var instruction in referenceBody.Instructions)
                {
                    var newOperand = instruction.Operand;
                    switch (instruction.Operand)
                    {
                        case MemberReference reference:
                        {
                            if (reference.IsField)
                            {
                                newOperand = module.DefaultImporter.ImportField(converter.Convert(reference));
                            } 
                            else if (reference.IsMethod)
                            {
                                newOperand = module.DefaultImporter.ImportMethod(converter.Convert(reference));
                            }
                            break;
                        }
                        case IMethodDescriptor descriptor:
                        {
                            newOperand = module.DefaultImporter.ImportMethod(converter.Convert(descriptor));
                            break;
                        }
                        case IFieldDescriptor descriptor:
                        {
                            newOperand = module.DefaultImporter.ImportField(converter.Convert(descriptor));
                            break;
                        }
                        case ITypeDefOrRef defOrRef:
                        {
                            newOperand = module.DefaultImporter.ImportType(converter.Convert(defOrRef));
                            break;
                        }
                        case CilInstructionLabel label:
                            newOperand = new CilOffsetLabel(label.Offset);
                            break;
                        case List<ICilLabel> list:
                            newOperand = list.Select(label => (ICilLabel) new CilOffsetLabel(label.Offset)).ToList();
                            break;
                        case null:
                        case sbyte:
                        case int:
                        case long: 
                        case float:
                        case double:
                        case string:
                        case Parameter:
                        case CilLocalVariable:
                            break;
                        default:
                            Console.WriteLine($"Unhandled operand type while copying method {method}: " + newOperand.GetType().FullName);
                            break;
                    }
                    body.Instructions.Add(new CilInstruction(instruction.OpCode, newOperand));
                }
                method.CilMethodBody = body;
            }
            catch (Exception e)
            {
                Console.WriteLine($"Error copying method: {method}");
                Console.WriteLine(e);
            }
        }
    }
}

var modules = new List<ModuleDefinition>();
var refModules = new List<ModuleDefinition?>();
var refToShimAssembly = new Dictionary<string, AssemblyDefinition>();
foreach (var path in inputPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.FromFile(path, readingParams);
    modules.Add(module);
    
    ModuleDefinition? referenceModule = null;
    foreach (var referencePath in referencePaths)
    {
        var referenceModulePath = referencePath + fileName;
        if (File.Exists(referenceModulePath))
        {
            if (referenceModule == null)
            {
                Console.WriteLine("Using reference at " + referenceModulePath);
                referenceModule = ModuleDefinition.FromFile(referenceModulePath);
                refToShimAssembly.Add(referenceModule.Assembly.Name, module.Assembly);
            }
        }
    }
    refModules.Add(referenceModule);
}

Console.WriteLine(refToShimAssembly);
foreach (var (key, value) in refToShimAssembly)
{
    Console.WriteLine($"{key}: {value}");
}

// wtf
FixIntPtrInModule(refToShimAssembly["mscorlib"].ManifestModule);

foreach (var (module, referenceModule) in modules.Zip(refModules))
{
    var importedDummy = module.DefaultImporter.ImportModule(dummyModule);
    foreach (var type in module.GetAllTypes())
    {
        ReferenceConverter converter = new ReferenceConverter(refToShimAssembly, module);
        SignatureComparer comparer = new SignatureComparer();
        ProcessType(type, module, importedDummy, referenceModule, converter, comparer);
    }
    
    module.Write(managedPath + module.Name);
}

Console.WriteLine("Done!");
