program Sniff;

uses
  Vcl.Forms,
  Vcl.Themes,
  Vcl.Styles,

  MSHeap in 'Core\MSHeap.pas',
  wbCommandLine in 'Core\wbCommandLine.pas',
  wbDataFormat in 'Core\wbDataFormat.pas',
  wbDataFormatMaterial in 'Core\wbDataFormatMaterial.pas',
  wbDataFormatNif in 'Core\wbDataFormatNif.pas',
  wbDataFormatNifTypes in 'Core\wbDataFormatNifTypes.pas',
  wbDataFormatMisc in 'Core\wbDataFormatMisc.pas',
  wbMeshOptimize in 'Core\wbMeshOptimize.pas',
  wbNifMath in 'Core\wbNifMath.pas',
  
  SniffProcessor in 'Sniff\SniffProcessor.pas',

  frmMain in 'Sniff\frmMain.pas' {FormMain},
  frMessages in 'Sniff\frMessages.pas' {FrameMessages: TFrame},
  frmVertexPaintHelper in 'Sniff\frmVertexPaintHelper.pas' {FormVertexPaintHelper},
  
  ProcAddBoundingBox in 'Sniff\ProcAddBoundingBox.pas' {FrameAddBoundingBox: TFrame},
  ProcAddFacialAnim in 'Sniff\ProcAddFacialAnim.pas' {FrameAddFacialAnim: TFrame},
  ProcAddHeadtrackingAnim in 'Sniff\ProcAddHeadtrackingAnim.pas' {FrameAddHeadtrackingAnim: TFrame},
  ProcAddLODNode in 'Sniff\ProcAddLODNode.pas' {FrameAddLODNode: TFrame},
  ProcAddRootCollisionNode in 'Sniff\ProcAddRootCollisionNode.pas' {FrameAddRootCollisionNode: TFrame},
  ProcAdjustTransform in 'Sniff\ProcAdjustTransform.pas' {FrameAdjustTransform: TFrame},
  ProcAnalyzeMesh in 'Sniff\ProcAnalyzeMesh.pas' {FrameAnalyzeMesh: TFrame},
  ProcAnimQuadraticToLinear in 'Sniff\ProcAnimQuadraticToLinear.pas' {FrameAnimQuadraticToLinear: TFrame},
  ProcAnimSkeletonDeath in 'Sniff\ProcAnimSkeletonDeath.pas' {FrameAnimSkeletonDeath: TFrame},
  ProcApplyTransform in 'Sniff\ProcApplyTransform.pas' {FrameApplyTransform: TFrame},
  ProcAttachParent in 'Sniff\ProcAttachParent.pas' {FrameAttachParent: TFrame},
  ProcChangePartitionSlot in 'Sniff\ProcChangePartitionSlot.pas' {FrameChangePartitionSlot: TFrame},
  ProcCheckForErrors in 'Sniff\ProcCheckForErrors.pas' {FrameCheckForErrors: TFrame},
  ProcConvertFO3Collision in 'Sniff\ProcConvertFO3Collision.pas' {FrameConvertFO3Collision: TFrame},
  ProcConvertRootNode in 'Sniff\ProcConvertRootNode.pas' {FrameConvertRootNode: TFrame},
  ProcConvertShader30 in 'Sniff\ProcConvertShader30.pas' {FrameConvertShader30: TFrame},
  ProcCopyControlledBlocks in 'Sniff\ProcCopyControlledBlocks.pas' {FrameCopyControlledBlocks: TFrame},
  ProcCopyGeometryBlocks in 'Sniff\ProcCopyGeometryBlocks.pas' {FrameCopyGeometryBlocks: TFrame},
  ProcCopyPriorities in 'Sniff\ProcCopyPriorities.pas' {FrameCopyPriorities: TFrame},
  ProcDismemberSkin in 'Sniff\ProcDismemberSkin.pas' {FrameDismemberSkin: TFrame},
  ProcFindDrawCalls in 'Sniff\ProcFindDrawCalls.pas' {FrameFindDrawCalls: TFrame},
  ProcFindSeveralStrips in 'Sniff\ProcFindSeveralStrips.pas' {FrameFindSeveralStrips: TFrame},
  ProcFindUVs in 'Sniff\ProcFindUVs.pas' {FrameFindUVs: TFrame},
  ProcFixExportedKFAnim in 'Sniff\ProcFixExportedKFAnim.pas' {FrameFixExportedKFAnim: TFrame},
  ProcGroupShapes in 'Sniff\ProcGroupShapes.pas' {FrameGroupShapes: TFrame},
  ProcHavokSearchMaterial in 'Sniff\ProcHavokSearchMaterial.pas' {FrameHavokMaterial: TFrame},
  ProcHavokSettingsUpdate in 'Sniff\ProcHavokSettingsUpdate.pas' {FrameHavokSettings: TFrame},
  ProcInertiaUpdate in 'Sniff\ProcInertiaUpdate.pas' {Frame1: TFrame},
  ProcJamAnim in 'Sniff\ProcJamAnim.pas' {FrameJamAnim: TFrame},
  ProcJsonConverter in 'Sniff\ProcJsonConverter.pas' {FrameJsonConverter: TFrame},
  ProcMergeShapes in 'Sniff\ProcMergeShapes.pas' {FrameMergeShapes: TFrame},
  ProcMoppUpdate in 'Sniff\ProcMoppUpdate.pas' {FrameMoppUpdate: TFrame},
  ProcOptimize in 'Sniff\ProcOptimize.pas' {FrameOptimize: TFrame},
  ProcOptimizeKF in 'Sniff\ProcOptimizeKF.pas' {FrameOptimizeKF: TFrame},
  ProcParallaxUpdate in 'Sniff\ProcParallaxUpdate.pas' {FrameParallaxUpdate: TFrame},
  ProcPriorityControlledBlocks in 'Sniff\ProcPriorityControlledBlocks.pas' {FramePriorityControlledBlocks: TFrame},
  ProcRagdollConstraintUpdate in 'Sniff\ProcRagdollConstraintUpdate.pas' {FrameRagdollConstraintUpdate: TFrame},
  ProcRemoveControlledBlocks in 'Sniff\ProcRemoveControlledBlocks.pas' {FrameRemoveControlledBlocks: TFrame},
  ProcRemoveNodes in 'Sniff\ProcRemoveNodes.pas' {FrameRemoveNodes: TFrame},
  ProcRemoveUnusedNodes in 'Sniff\ProcRemoveUnusedNodes.pas' {FrameRemoveUnusedNodes: TFrame},
  ProcRenameControlledBlocks in 'Sniff\ProcRenameControlledBlocks.pas' {FrameRenameControlledBlocks: TFrame},
  ProcRenameStrings in 'Sniff\ProcRenameStrings.pas' {FrameRenameStrings: TFrame},
  ProcReplaceAssets in 'Sniff\ProcReplaceAssets.pas' {FrameReplaceAssets: TFrame},
  ProcSetMissingNames in 'Sniff\ProcSetMissingNames.pas' {FrameSetMissingNames: TFrame},
  ProcShaderFlagsUpdate in 'Sniff\ProcShaderFlagsUpdate.pas' {FrameShaderFlagsUpdate: TFrame},
  ProcTangents in 'Sniff\ProcTangents.pas' {FrameTangents: TFrame},
  ProcUniversalFixer in 'Sniff\ProcUniversalFixer.pas' {FrameUniversalFixer: TFrame},
  ProcUniversalTweaker in 'Sniff\ProcUniversalTweaker.pas' {FrameUniversalTweaker: TFrame},
  ProcUnskinMesh in 'Sniff\ProcUnskinMesh.pas' {FrameUnskinMesh: TFrame},
  ProcUnweldedVertices in 'Sniff\ProcUnweldedVertices.pas' {FrameUnweldedVertices: TFrame},
  ProcUpdateAuthor in 'Sniff\ProcUpdateAuthor.pas' {FrameUpdateAuthor: TFrame},
  ProcUpdateBounds in 'Sniff\ProcUpdateBounds.pas' {FrameUpdateBounds: TFrame},
  ProcVertexPaint in 'Sniff\ProcVertexPaint.pas' {FrameVertexPaint: TFrame},
  ProcWallsReflectionFlag in 'Sniff\ProcWallsReflectionFlag.pas' {FrameWallsReflectionFlag: TFrame},
  ProcWeiExplosion in 'Sniff\ProcWeiExplosion.pas' {FrameWeiExplosion: TFrame};
  
{$R *.res}

{$DYNAMICBASE ON}
{$SetPEFlags $0020}
begin
  try
  Application.Initialize;
  Application.MainFormOnTaskbar := True;
  Application.CreateForm(TFormMain, FormMain);
  Application.Run;
  except end;
end.
