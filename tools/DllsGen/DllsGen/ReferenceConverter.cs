using AsmResolver.DotNet;
using AsmResolver.DotNet.Signatures;
using AsmResolver.DotNet.Signatures.Types;

namespace DllsGen;

public class ReferenceConverter
{
    private Dictionary<string, AssemblyDefinition> _refToShimAssembly;
    private ModuleDefinition _module;

    public ReferenceConverter(Dictionary<string, AssemblyDefinition> refToShimAssembly, ModuleDefinition module)
    {
        _refToShimAssembly = refToShimAssembly;
        _module = module;
    }

    public TypeReference Convert(TypeReference reference)
    {
        var assemblyName = reference.Resolve().Module.Assembly.Name;
        var shimAssembly = _refToShimAssembly[assemblyName];
        IResolutionScope scope = _module.Assembly == shimAssembly ? _module : new AssemblyReference(_refToShimAssembly[assemblyName]);
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

    public TypeSignature Convert(TypeSignature anySig)
    {
        switch (anySig)
        {
            case CorLibTypeSignature sig:
                return _module.CorLibTypeFactory.FromElementType(sig.ElementType);
            case PointerTypeSignature sig:
                return new PointerTypeSignature(Convert(sig.BaseType));
            case ByReferenceTypeSignature sig:
                return new ByReferenceTypeSignature(Convert(sig.BaseType));
            case TypeDefOrRefSignature sig:
                return new TypeDefOrRefSignature(Convert(sig.Type));
            case GenericParameterSignature sig:
                return new GenericParameterSignature(sig.ParameterType, sig.Index);
            case ArrayTypeSignature sig:
                return new ArrayTypeSignature(Convert(sig.BaseType), sig.Dimensions.ToArray());
            case GenericInstanceTypeSignature sig:
                return new GenericInstanceTypeSignature(Convert(sig.GenericType), sig.IsValueType,
                    sig.TypeArguments.Select(Convert).ToArray());
            case FunctionPointerTypeSignature sig:
                return new FunctionPointerTypeSignature(Convert(sig.Signature));
            case SzArrayTypeSignature sig:
                return new SzArrayTypeSignature(Convert(sig.BaseType));
            case CustomModifierTypeSignature sig:
                return new CustomModifierTypeSignature(Convert(sig.ModifierType), sig.IsRequired,
                    Convert(sig.BaseType));
            case SentinelTypeSignature sig:
                return sig;
            default:
                throw new Exception();
        }
    }

    public MethodSignature Convert(MethodSignature sig)
    {
        return new MethodSignature(sig.Attributes, Convert(sig.ReturnType), sig.ParameterTypes.Select(Convert))
        {
            GenericParameterCount = sig.GenericParameterCount
        };
    }
    
    public IMethodDescriptor Convert(IMethodDescriptor descriptor)
    {
        return null;
    }
}
