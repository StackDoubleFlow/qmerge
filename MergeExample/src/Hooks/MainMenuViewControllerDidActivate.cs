using QMerge.Hooking;
using UnityEngine;

namespace MergeExample.Hooks
{
    [Hook(typeof(MainMenuViewController), "DidActivate")]
    public class MainMenuViewControllerDidActivate
    {
        public static void Postfix(MusicPackPromoBanner ____musicPackPromoBanner)
        {
            Debug.Log("in postfix");
            Debug.Log(____musicPackPromoBanner);
            ____musicPackPromoBanner.gameObject.SetActive(false);
            Debug.Log("done with postfix");
        } 
            
    }
}