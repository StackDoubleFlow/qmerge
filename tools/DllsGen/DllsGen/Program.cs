using AsmResolver.DotNet;
using AsmResolver.DotNet.Code.Cil;
using AsmResolver.DotNet.Collections;
using AsmResolver.DotNet.Serialized;
using AsmResolver.DotNet.Signatures;
using AsmResolver.DotNet.Signatures.Types;
using AsmResolver.PE.DotNet.Cil;
using AsmResolver.PE.DotNet.Metadata.Tables.Rows;

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

static void ProcessType(TypeDefinition type, ModuleDefinition module, ModuleReference dummyModule, ModuleDefinition? referenceModule)
{
    var referenceTypeRef = referenceModule?.CreateTypeReference(type.Namespace, type.Name);
    var referenceType = referenceModule?.MetadataResolver.ResolveType(referenceTypeRef);

    foreach (var method in type.Methods)
    {
        if (method.IsPInvokeImpl)
        {
            const ImplementationMapAttributes attributes = ImplementationMapAttributes.NoMangle | ImplementationMapAttributes.CharSetAuto | ImplementationMapAttributes.CallConvCdecl;
            method.ImplementationMap = new ImplementationMap(dummyModule, "MergePInvokeDummy", attributes);
        }

        if (type.GenericParameters.Count > 0 || method.GenericParameters.Count > 0)
        {
            var referenceMethod = (MethodDefinition?) referenceType?.CreateMemberReference(method.Name, method.Signature).Resolve();
            if (referenceMethod?.CilMethodBody == null) continue;
        
            var body = new CilMethodBody(method);
            var referenceBody = referenceMethod.CilMethodBody;

            body.MaxStack = referenceBody.MaxStack;
            body.ComputeMaxStackOnBuild = false;
            body.InitializeLocals = referenceBody.InitializeLocals;
            
            foreach (var localVariable in referenceBody.LocalVariables)
            {
                body.LocalVariables.Add(new CilLocalVariable(localVariable.VariableType.ImportWith(module.DefaultImporter)));
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
                    ExceptionType = exceptionHandler.ExceptionType?.ImportWith(module.DefaultImporter),
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
                                newOperand = module.DefaultImporter.ImportField(((IFieldDescriptor) reference).Resolve());
                            } 
                            else if (reference.IsMethod)
                            {
                                newOperand = module.DefaultImporter.ImportMethod(((IMethodDescriptor) reference).Resolve());
                            }
                            break;
                        }
                        case IMethodDescriptor descriptor:
                        {
                            newOperand = module.DefaultImporter.ImportMethod(descriptor);
                            break;
                        }
                        case IFieldDescriptor descriptor:
                        {
                            newOperand = module.DefaultImporter.ImportField(descriptor);
                            break;
                        }
                        case TypeSpecification spec:
                        {
                            newOperand = spec.ImportWith(module.DefaultImporter);
                            break;
                        }
                        case ITypeDescriptor def:
                        {
                            newOperand = module.DefaultImporter.ImportType(def.Resolve());
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

foreach (var path in inputPaths)
{
    var fileName = Path.GetFileName(path);
    var module = ModuleDefinition.FromFile(path, readingParams);

    ModuleDefinition? referenceModule = null;
    foreach (var referencePath in referencePaths)
    {
        var referenceModulePath = referencePath + fileName;
        if (File.Exists(referenceModulePath))
        {
            Console.WriteLine("Using reference at " + referenceModulePath);
            referenceModule = ModuleDefinition.FromFile(referenceModulePath);
            break;
        }
    }

    var importedDummy = module.DefaultImporter.ImportModule(dummyModule);
    foreach (var type in module.GetAllTypes())
    {
        ProcessType(type, module, importedDummy, referenceModule);
    }
    
    module.Write(managedPath + fileName);
}

Console.WriteLine("Done!");
