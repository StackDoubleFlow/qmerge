using AsmResolver.DotNet;
using AsmResolver.DotNet.Signatures;
using AsmResolver.DotNet.Signatures.Types;

namespace DllsGen;

public class ReferenceConverter
{
    private readonly Dictionary<string, AssemblyDefinition> _refToShimAssembly;
    private readonly ModuleDefinition _module;

    public ReferenceConverter(Dictionary<string, AssemblyDefinition> refToShimAssembly, ModuleDefinition module)
    {
        _refToShimAssembly = refToShimAssembly;
        _module = module;
    }

    public ModuleReference Convert(ModuleReference reference)
    {
        return new ModuleReference(reference.Name);
    }

    public TypeReference Convert(TypeReference reference)
    {
        var assemblyName = reference.Resolve().Module.Assembly.Name;
        var shimAssembly = _refToShimAssembly[assemblyName];
        IResolutionScope scope = _module.Assembly == shimAssembly ? _module : _module.DefaultImporter.ImportScope(new AssemblyReference(_refToShimAssembly[assemblyName]));
        return new TypeReference(_module, scope, reference.Namespace, reference.Name);
    }

    public ITypeDefOrRef? Convert(ITypeDefOrRef typeDefOrRef)
    {
        switch (typeDefOrRef)
        {
            case TypeDefinition typeDef:
                return new DefaultMetadataResolver(_module.MetadataResolver.AssemblyResolver).ResolveType(Convert(typeDef.ToTypeReference()));
            case TypeReference typeRef:
                return Convert(typeRef);
            case TypeSpecification typeSpec:
                return new TypeSpecification(Convert(typeSpec.Signature));
        }
        Console.WriteLine(typeDefOrRef.GetType().FullName);
        return null;
    }

    public TypeSignature Convert(TypeSignature anySig) => anySig switch
    {
        CorLibTypeSignature sig => _module.CorLibTypeFactory.FromElementType(sig.ElementType),
        PointerTypeSignature sig => new PointerTypeSignature(Convert(sig.BaseType)),
        ByReferenceTypeSignature sig => new ByReferenceTypeSignature(Convert(sig.BaseType)),
        TypeDefOrRefSignature sig => new TypeDefOrRefSignature(Convert(sig.Type)),
        GenericParameterSignature sig => new GenericParameterSignature(sig.ParameterType, sig.Index),
        ArrayTypeSignature sig => new ArrayTypeSignature(Convert(sig.BaseType), sig.Dimensions.ToArray()),
        GenericInstanceTypeSignature sig => new GenericInstanceTypeSignature(Convert(sig.GenericType), sig.IsValueType,
            sig.TypeArguments.Select(Convert).ToArray()),
        FunctionPointerTypeSignature sig => new FunctionPointerTypeSignature(Convert(sig.Signature)),
        SzArrayTypeSignature sig => new SzArrayTypeSignature(Convert(sig.BaseType)),
        CustomModifierTypeSignature sig => new CustomModifierTypeSignature(Convert(sig.ModifierType), sig.IsRequired,
            Convert(sig.BaseType)),
        SentinelTypeSignature sig => sig,
        _ => throw new Exception(),
    };

    public FieldSignature Convert(FieldSignature sig)
    {
        return new FieldSignature(sig.Attributes, Convert(sig.FieldType));
    }

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

    public MethodSignature Convert(MethodSignature sig)
    {
        return new MethodSignature(sig.Attributes, Convert(sig.ReturnType), sig.ParameterTypes.Select(Convert))
        {
            GenericParameterCount = sig.GenericParameterCount
        };
    }

    public GenericInstanceMethodSignature Convert(GenericInstanceMethodSignature sig)
    {
        return new GenericInstanceMethodSignature(sig.Attributes, sig.TypeArguments.Select(Convert).ToArray());
    }
    
    public IMethodDescriptor Convert(IMethodDescriptor descriptor) => descriptor switch
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
