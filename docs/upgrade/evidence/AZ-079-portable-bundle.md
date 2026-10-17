# AZ-079 可移植候选源码包

2026-10-06，Codex。重建脚本新增candidate.tar产物，排除临时.git元数据，清单记录SHA256；源码仍由固定提交和两个审核补丁构成，不读取邻仓未提交文件。归档无需邻仓即可提取，生成的Cargo配置仍指向生成目录；产品依赖尚未切换。

cycle039实际生成及校验退出0，session8096。归档SHA256 cdb9c88fd4c91c5033bda9c1f1cc166e74afbe4bbb4aa3d14ad84fa6a8459065。实际提取到.cache/kernel-candidate-69f93a49-a6a5-4808-b61e-91ada7ef4947/relocated，无.git目录，cargo metadata --no-deps解析独立工作区成功；结果.cache/upgrade-kernel-bundle-metadata-039.json。归档摘要只标识此次产物，未宣称不同生成时间的tar字节可重复。

已测重建候选核心4/4、宿主18/18、路由2/2另见AZ-079-reconstructed-validation.md。本次迁移只做清单解析，未重复编译迁移目录，未验证跨平台与产品原生点击。下一步把可审查固定源码依赖接入产品，再跑接入回归；AZ-079保持doing，不能把缓存归档当成产品已修复或已发布。
