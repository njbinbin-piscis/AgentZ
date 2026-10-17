import { expect, it } from "vitest";
import { EditorConflictTracker } from "./editorConflict";
function fixture(dirty = false) { const tracker = new EditorConflictTracker(); tracker.setScope("project"); tracker.open("file", "original", dirty); return tracker; }
it("only latest read request can apply and repeated result is consumed", () => {
  const tracker = fixture(); const first = tracker.beginRead("file")!; const latest = tracker.beginRead("file")!;
  expect(tracker.settleRead(first,{kind:"content",content:"old"})).toEqual({status:"stale"});
  expect(tracker.settleRead(latest,{kind:"content",content:"latest"})).toEqual({status:"applied",content:"latest"});
  expect(tracker.settleRead(latest,{kind:"content",content:"duplicate"})).toEqual({status:"stale"}); expect(tracker.snapshot("file")?.content).toBe("latest");
});
it("ABA same content local edits reject old reload and preserve dirty revision", () => {
  const tracker=fixture();const captured=tracker.beginRead("file")!;tracker.edit("file","new");tracker.edit("file","original");
  expect(tracker.settleRead(captured,{kind:"content",content:"disk"})).toEqual({status:"stale"});expect(tracker.snapshot("file")?.isDirty).toBe(true);expect(tracker.snapshot("file")?.localRevision).toBe(2);
});
it("closed and reopened same path/content is a different tab even in the same project", () => {
  const tracker=fixture();const captured=tracker.beginRead("file")!;tracker.close("file");tracker.open("file","original");
  expect(tracker.settleRead(captured,{kind:"content",content:"late"})).toEqual({status:"stale"});expect(tracker.snapshot("file")?.tabIdentity).not.toBe(captured.tabIdentity);
});
it("project switch ABA rejects captured reads despite same path/content", () => {
  const tracker=fixture();const captured=tracker.beginRead("file")!;tracker.setScope("other");tracker.open("file","original");tracker.setScope("project");tracker.open("file","original");
  expect(tracker.settleRead(captured,{kind:"content",content:"late"})).toEqual({status:"stale"});
});
it("save after typing cannot mark newer edits clean, and older save failure cannot replace state", () => {
  const tracker=fixture(true);const captured=tracker.beginSave("file")!;tracker.edit("file","new typing");
  expect(tracker.settleSave(captured,true)).toEqual({status:"stale"});expect(tracker.snapshot("file")?.isDirty).toBe(true);expect(tracker.snapshot("file")?.content).toBe("new typing");
  const latest=tracker.beginSave("file")!;expect(tracker.settleSave(latest,false)).toEqual({status:"failed"});expect(tracker.snapshot("file")?.isDirty).toBe(true);
});
it("dirty deletion, unreadable disk and external content preserve local buffer", () => {
  const tracker=fixture(true);const before=tracker.snapshot("file");
  expect(tracker.settleRead(tracker.beginRead("file")!,{kind:"deleted"})).toEqual({status:"deleted",localContent:"original",preserveBuffer:true});
  expect(tracker.settleRead(tracker.beginRead("file")!,{kind:"error"})).toEqual({status:"unavailable",localContent:"original",isDirty:true});
  expect(tracker.settleRead(tracker.beginRead("file")!,{kind:"content",content:"external"})).toEqual({status:"conflict",localContent:"original",diskContent:"external"});expect(tracker.snapshot("file")).toEqual(before);
});
it("current save alone clears dirty and invalidates pending read, without writing a file", () => {
  const tracker=fixture(true);const read=tracker.beginRead("file")!;const save=tracker.beginSave("file")!;
  expect(tracker.settleSave(save,true)).toEqual({status:"saved"});expect(tracker.snapshot("file")?.isDirty).toBe(false);expect(tracker.settleRead(read,{kind:"content",content:"old"})).toEqual({status:"stale"});expect(tracker.settleSave(save,true)).toEqual({status:"stale"});
});
it("explicit confirmed discard applies only the captured dirty revision", () => {
  const tracker=fixture(true);const captured=tracker.beginRead("file",{allowDiscard:true})!;
  expect(tracker.snapshot("file")?.isDirty).toBe(true);
  expect(tracker.settleRead(captured,{kind:"content",content:"disk"})).toEqual({status:"applied",content:"disk"});
  expect(tracker.snapshot("file")?.isDirty).toBe(false);
  tracker.edit("file","local");const confirmed=tracker.beginRead("file",{allowDiscard:true})!;tracker.edit("file","local");
  expect(tracker.settleRead(confirmed,{kind:"content",content:"late"})).toEqual({status:"stale"});expect(tracker.snapshot("file")?.content).toBe("local");expect(tracker.snapshot("file")?.isDirty).toBe(true);
});
it("confirmed discard read failure or deletion does not clear dirty content", () => {
  const tracker=fixture(true);const before=tracker.snapshot("file");
  expect(tracker.settleRead(tracker.beginRead("file",{allowDiscard:true})!,{kind:"error"})).toEqual({status:"unavailable",localContent:"original",isDirty:true});
  expect(tracker.settleRead(tracker.beginRead("file",{allowDiscard:true})!,{kind:"deleted"})).toEqual({status:"deleted",localContent:"original",preserveBuffer:true});
  expect(tracker.snapshot("file")).toEqual(before);
});
it("clean to dirty same-content edit rejects pending read without requiring content change", () => {
  const tracker=fixture();const clean=tracker.snapshot("file")!;const read=tracker.beginRead("file")!;
  tracker.edit("file",clean.content);
  expect(tracker.settleRead(read,{kind:"content",content:"external"})).toEqual({status:"stale"});
  expect(tracker.snapshot("file")).toMatchObject({content:clean.content,isDirty:true,localRevision:clean.localRevision+1});
});
it("confirmed reload and successful save each invalidate the other captured operation", () => {
  const tracker=fixture(true);const oldSave=tracker.beginSave("file")!;
  expect(tracker.settleRead(tracker.beginRead("file",{allowDiscard:true})!,{kind:"content",content:"disk"})).toEqual({status:"applied",content:"disk"});
  expect(tracker.settleSave(oldSave,true)).toEqual({status:"stale"});
  const read=tracker.beginRead("file")!;const save=tracker.beginSave("file")!;
  expect(tracker.settleSave(save,true)).toEqual({status:"saved"});
  expect(tracker.settleRead(read,{kind:"content",content:"older disk"})).toEqual({status:"stale"});
  expect(tracker.snapshot("file")?.content).toBe("disk");
});
it("old save failure and read failure cannot clear a newer dirty edit", () => {
  const tracker=fixture();const save=tracker.beginSave("file")!;const read=tracker.beginRead("file")!;
  tracker.edit("file","new draft");const draft=tracker.snapshot("file");
  expect(tracker.settleSave(save,false)).toEqual({status:"stale"});
  expect(tracker.settleRead(read,{kind:"error"})).toEqual({status:"stale"});
  expect(tracker.snapshot("file")).toEqual(draft);
  expect(tracker.settleRead(tracker.beginRead("file")!,{kind:"error"})).toEqual({status:"unavailable",localContent:"new draft",isDirty:true});
  expect(tracker.snapshot("file")).toEqual(draft);
});
