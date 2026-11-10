// Reads the "Recent" list of an application's taskbar jump list (IApplicationDocumentLists), so
// inside.ps1 can check that files Specular views are filed under the AppUserModelID its
// Start-menu shortcut carries. Compiled by PowerShell's Add-Type inside Windows Sandbox.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class SpecularJumpList
{
    [ComImport, Guid("3c594f9f-9f30-47a1-979a-c9e83d3d0a06"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IApplicationDocumentLists
    {
        void SetAppID([MarshalAs(UnmanagedType.LPWStr)] string appId);
        [PreserveSig]
        int GetList(int listType, uint itemsWanted, ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object list);
    }

    [ComImport, Guid("92CA9DCD-5622-4bba-A805-5E9F541BD8C9"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IObjectArray
    {
        void GetCount(out uint count);
        void GetAt(uint index, ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object item);
    }

    [ComImport, Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellItem
    {
        void BindToHandler(IntPtr bindCtx, ref Guid bhid, ref Guid riid, out IntPtr result);
        void GetParent(out IShellItem parent);
        void GetDisplayName(uint sigdn, [MarshalAs(UnmanagedType.LPWStr)] out string name);
        void GetAttributes(uint mask, out uint attributes);
        void Compare(IShellItem other, uint hint, out int order);
    }

    const int ADLT_RECENT = 0;
    const uint SIGDN_FILESYSPATH = 0x80058000;

    /// The file paths in `appId`'s jump list "Recent" category, newest first.
    public static string[] Recent(string appId)
    {
        Type type = Type.GetTypeFromCLSID(new Guid("86bec222-30f2-47e0-9f25-60d11cd75c28"));
        var lists = (IApplicationDocumentLists)Activator.CreateInstance(type);
        lists.SetAppID(appId);
        Guid arrayId = new Guid("92CA9DCD-5622-4bba-A805-5E9F541BD8C9");
        object found;
        if (lists.GetList(ADLT_RECENT, 0, ref arrayId, out found) != 0 || found == null)
        {
            return new string[0];
        }
        var items = (IObjectArray)found;
        uint count;
        items.GetCount(out count);
        Guid itemId = new Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe");
        var paths = new List<string>();
        for (uint i = 0; i < count; i++)
        {
            try
            {
                object item;
                items.GetAt(i, ref itemId, out item);
                string path;
                ((IShellItem)item).GetDisplayName(SIGDN_FILESYSPATH, out path);
                paths.Add(path);
            }
            catch (COMException)
            {
            }
        }
        return paths.ToArray();
    }
}
