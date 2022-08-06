using AsmResolver.DotNet;
using AsmResolver.DotNet.Signatures;
using AsmResolver.PE.DotNet.Metadata.Tables.Rows;

namespace DllsGen;

public class ModuleProcessor
{
    private ReferenceConverter _converter;
    private SignatureComparer _comparer;
    private ModuleDefinition _module;
    private ModuleDefinition? _referenceModule;
    private ModuleReference _dummyModule;

    public ModuleProcessor(ModuleDefinition module, ModuleDefinition? referenceModule, ModuleReference dummyModule,
        Dictionary<string, AssemblyDefinition> refToShimAssemblyMap)
    {
        _module = module;
        _referenceModule = referenceModule;
        _comparer = new SignatureComparer();
        _converter = new ReferenceConverter(refToShimAssemblyMap, module);
        _dummyModule = module.DefaultImporter.ImportModule(dummyModule);
    }

    public void Process()
    {
        foreach (var type in _module.GetAllTypes())
        {
            ProcessType(type);
        }
    }

    private void ProcessType(TypeDefinition type)
    {
        if (type.Namespace == "Cpp2IlInjected")
            return;
        var referenceType = FindTypeInModule(type, _referenceModule);
        if (referenceType == null)
            Console.WriteLine("Could not find reference type for " + type);
        //
        // foreach (var nestedType in type.NestedTypes)
        // {
        //     nestedType.IsPublic = true;
        // }

        // These custom attributes get stripped from the il2cpp binaries but are useful to have, so we'll copy them from the
        // reference assemblies.
        if (referenceType != null && type.CustomAttributes.Count != referenceType.CustomAttributes.Count)
        {
            foreach (var ca in referenceType.CustomAttributes)
            {
                if (ca.Constructor.FullName ==
                    "System.Void System.Reflection.DefaultMemberAttribute::.ctor(System.String)" ||
                    ca.Constructor.DeclaringType.FullName == "System.AttributeUsageAttribute")
                {
                    type.CustomAttributes.Add(
                        new CustomAttribute((ICustomAttributeType) _converter.Convert(ca.Constructor), ca.Signature));
                }
            }
        }

        foreach (var method in type.Methods)
        {
            // Il2Cpp is not going to find the module for p/invoke implementations, so we'll use our dummy module
            if (method.IsPInvokeImpl)
            {
                const ImplementationMapAttributes attributes = ImplementationMapAttributes.NoMangle |
                                                               ImplementationMapAttributes.CharSetAuto |
                                                               ImplementationMapAttributes.CallConvCdecl;
                method.ImplementationMap = new ImplementationMap(_dummyModule, "MergePInvokeDummy", attributes);
            }

            // For mods to be able to call generic methods, they need implementations so il2cpp can instantiate them. Here
            // we can try to find a matching generic method in the reference assemblies and copy its method body.
            if (type.GenericParameters.Count > 0 || method.GenericParameters.Count > 0)
            {
                var referenceMethod = (MethodDefinition?) referenceType
                    ?.CreateMemberReference(method.Name, method.Signature.ImportWith(_referenceModule.DefaultImporter))
                    .Resolve();
                if (referenceType != null && referenceMethod == null)
                {
                    foreach (var referenceTypeMethod in referenceType.Methods)
                    {
                        if (referenceTypeMethod.Name == method.Name)
                        {
                            try
                            {
                                var convertedSig = _converter.Convert(referenceTypeMethod.Signature);
                                if (_comparer.Equals(convertedSig, method.Signature))
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

                if (referenceMethod == null) continue;

                // Looks like either Cpp2Il doesn't get the right impl attributes or Il2Cpp doesn't properly write them.
                // Either way, we can copy them from the reference method.
                method.ImplAttributes = referenceMethod.ImplAttributes;

                if (referenceMethod.CilMethodBody == null) continue;
                try
                {
                    MethodCopier.CopyMethodBody(referenceMethod, method, _converter);
                }
                catch (Exception e)
                {
                    Console.WriteLine($"Error copying method: {method}");
                    Console.WriteLine(e);
                }
            }
        }
    }

    private static TypeDefinition? FindTypeInModule(TypeDefinition type, ModuleDefinition module)
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
}