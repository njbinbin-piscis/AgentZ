# AZ041 实施复核检查点

2026-10-10。主实现为未保存缓冲区保护，AZ064新工作流包已归档；本轮源码变动不在该包中。

纯身份模块9项fixtures及lint实际通过，初轮index最终TSC/lint也通过。但独立复核发现热退出恢复在异步读取期间仅凭当前tab clean状态，用旧恢复draft替换同path仍可能覆盖用户期间编辑后保存或关闭重开版本；当前clean不等于无新意图。另centralsetter须处理同内容clean→dirty状态转换，防旧pending read接纳。

上述问题已退回接线作者修复，独立审查继续，不冻结候选、不宣称AZ041完整验收。要求保留当前版本与未采纳恢复draft，不允许恢复草稿被新snapshot静默抹掉。共享AZ036源码pins待最终独立复核后原子刷新，历史审查不篡改。宿主写CAS、原生Monaco/重启/Agent并发和跨平台验收仍未执行。
## 最终修复与实际复跑
恢复竞态退回后，当前同path版本始终保留，旧dirty稿成为独立只读恢复标签，真实snapshot保存正文；reserved恢复路径不进入磁盘读写/Monaco/LSP/Agent，显示专用textarea/复制，显式关闭说明只丢弃备份不保存原文件。磁盘变binary仍保留原dirty稿；同内容clean→dirty同步revision失效旧read。

Root实际 Session43548 退出0：editorConflict12 + editorRecovery真实helper/serializer集成4 + navigation6 = 22 passed，4.36s。作者最终fullTSC Session83900退出0；scopedlint0errors/8既有warnings。5份源manifest已冻结，独立review须匹配新增12fixture最终SHA，再原子刷新AZ036共享index及AZ041自身pins。完整GUI/OS重启、hostCAS和显式比较/合并仍未验收，不标done或稳定发行。最新安装器是前轮freeze，包含本轮改动的包尚未构建。
## 台账原子复核完成
AZ017/AZ036共享翻译文件出现未归因指纹漂移；保留原文件，不覆盖用户变动，不假称仅换行或全部语义相同。独立限定复核当前导航/隔离说明消费keys后，结合AZ041最终5源复核，原子刷新017/036/041记录成功；实际validate通过83tasks/11milestones。旧复核与指纹保留history。主后续只读评估AZ043调试会话生命周期，完整AZ041 hostCAS/比较合并/原生恢复保留后续。
