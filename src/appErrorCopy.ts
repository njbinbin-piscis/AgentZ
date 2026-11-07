/** Copy for the last-resort window boundary, which renders outside i18n. */
export function appErrorCopy(language: string) {
  if (language.toLowerCase().startsWith("zh")) {
    return {
      title: "界面在恢复时出错",
      body: "上次打开的文件或预览让界面崩溃了。可以不恢复那些文件，重新进入项目。",
      reload: "重新加载",
      skip: "不恢复已打开的文件",
    };
  }
  return {
    title: "The window failed while restoring",
    body: "A file or preview from the last session crashed the window. You can reopen the project without those files.",
    reload: "Reload",
    skip: "Reopen without restored files",
  };
}
