using QMerge.Hooking;

namespace MergeExample.Hooks
{
    [Hook(typeof(MainMenuViewController), "DidActivate")]
    public class MainMenuViewControllerDidActivate
    {
        public static void Postfix(ref MusicPackPromoBanner ____musicPackPromoBanner) => ____musicPackPromoBanner.gameObject.SetActive(false);
    }
}