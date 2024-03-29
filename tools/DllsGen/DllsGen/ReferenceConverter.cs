using AsmResolver.DotNet;
using AsmResolver.DotNet.Signatures;
using AsmResolver.DotNet.Signatures.Types;

namespace DllsGen;

/// <summary>
/// Convert references from one set of assemblies to references from another set of assembles.
/// </summary>
public class ReferenceConverter
{
    private readonly Dictionary<string, AssemblyDefinition> _assemblyNameMap;
    private readonly ModuleDefinition _module;

    /// <param name="assemblyNameMap">a map from the name of the assemblies we're converting from to the target assemblies.</param>
    /// <param name="module">the module which the new references will belong to.</param>
    public ReferenceConverter(Dictionary<string, AssemblyDefinition> assemblyNameMap, ModuleDefinition module)
    {
        _assemblyNameMap = assemblyNameMap;
        _module = module;
    }

    public ModuleReference Convert(ModuleReference reference) => new(reference.Name);

    public AssemblyReference MakeAssemblyReference(string name)
    {
        return (AssemblyReference) _module.DefaultImporter.ImportScope(new AssemblyReference(_assemblyNameMap[name]));
    }

    public IResolutionScope Convert(IResolutionScope scope) => scope switch
    {
        AssemblyReference assemblyReference => MakeAssemblyReference(assemblyReference.Name),
        ModuleDefinition _ => _module,
        TypeReference typeRef => Convert(typeRef),
        _ => throw new Exception()
    };

    public TypeReference Convert(TypeReference reference)
    {
        var def = reference.Resolve();
        var scope = def?.DeclaringType == null && def.Module.Name != _module.Name
            ? MakeAssemblyReference(def.Module.Assembly.Name)
            : Convert(reference.Scope);
        return new TypeReference(_module, scope, reference.Namespace, reference.Name);
    }

    public ITypeDefOrRef? Convert(ITypeDefOrRef typeDefOrRef) => typeDefOrRef switch
    {
        TypeDefinition typeDef => _module.MetadataResolver.ResolveType(Convert(typeDef.ToTypeReference())),
        TypeReference typeRef => Convert(typeRef),
        TypeSpecification typeSpec => new TypeSpecification(Convert(typeSpec.Signature)),
        _ => throw new Exception()
    };

    public TypeSignature Convert(TypeSignature anySig) => anySig switch
    {
        CorLibTypeSignature sig => _module.CorLibTypeFactory.FromElementType(sig.ElementType),
        PointerTypeSignature sig => new PointerTypeSignature(Convert(sig.BaseType)),
        ByReferenceTypeSignature sig => new ByReferenceTypeSignature(Convert(sig.BaseType)),
        TypeDefOrRefSignature sig => new TypeDefOrRefSignature(Convert(sig.Type)),
        GenericParameterSignature sig => new GenericParameterSignature((ModuleDefinition) Convert(sig.Scope),
            sig.ParameterType, sig.Index),
        ArrayTypeSignature sig => new ArrayTypeSignature(Convert(sig.BaseType), sig.Dimensions.ToArray()),
        GenericInstanceTypeSignature sig => new GenericInstanceTypeSignature(Convert(sig.GenericType), sig.IsValueType,
            sig.TypeArguments.Select(Convert).ToArray()),
        FunctionPointerTypeSignature sig => new FunctionPointerTypeSignature(Convert(sig.Signature)),
        SzArrayTypeSignature sig => new SzArrayTypeSignature(Convert(sig.BaseType)),
        // Custom modifiers aren't emmited in shim assemblies
        // CustomModifierTypeSignature sig => new CustomModifierTypeSignature(Convert(sig.ModifierType), sig.IsRequired,
        //     Convert(sig.BaseType)),
        CustomModifierTypeSignature sig => Convert(sig.BaseType),
        SentinelTypeSignature sig => sig,
        _ => throw new Exception(),
    };

    public FieldSignature Convert(FieldSignature sig) => new(sig.Attributes, Convert(sig.FieldType));

    public IFieldDescriptor Convert(IFieldDescriptor descriptor) => descriptor switch
    {
        FieldDefinition def => (FieldDefinition) Convert(def.DeclaringType)
            .CreateMemberReference(def.Name, Convert(def.Signature))
            .Resolve(),
        _ => throw new Exception()
    };

    public MemberReference Convert(MemberReference reference)
    {
        IMemberRefParent? parent = reference.Parent switch
        {
            ITypeDefOrRef typeDefOrRef => Convert(typeDefOrRef),
            ModuleReference moduleReference => Convert(moduleReference),
            _ => throw new Exception()
        };
        MemberSignature sig = reference.Signature switch
        {
            MethodSignature methodSignature => Convert(methodSignature),
            FieldSignature fieldSignature => Convert(fieldSignature),
            _ => throw new Exception()
        };
        return new MemberReference(parent, reference.Name, sig);
    }

    public MethodSignature Convert(MethodSignature sig) =>
        new(sig.Attributes, Convert(sig.ReturnType), sig.ParameterTypes.Select(Convert))
        {
            GenericParameterCount = sig.GenericParameterCount
        };

    public GenericInstanceMethodSignature Convert(GenericInstanceMethodSignature sig)
    {
        return new GenericInstanceMethodSignature(sig.Attributes, sig.TypeArguments.Select(Convert).ToArray());
    }

    public IMethodDescriptor? Convert(IMethodDescriptor descriptor) => descriptor switch
    {
        MethodDefinition def => (MethodDefinition) Convert(def.DeclaringType)
            .CreateMemberReference(def.Name, Convert(def.Signature))
            .Resolve(),
        MethodSpecification spec => new MethodSpecification((IMethodDefOrRef) Convert(spec.Method),
            Convert(spec.Signature)),
        MemberReference reference => Convert(reference),
        _ => throw new Exception()
    };
}