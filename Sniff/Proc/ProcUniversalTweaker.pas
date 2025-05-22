{******************************************************************************

  This Source Code Form is subject to the terms of the Mozilla Public License,
  v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain
  one at https://mozilla.org/MPL/2.0/.

*******************************************************************************}

unit ProcUniversalTweaker;

interface

uses
  Winapi.Windows, Winapi.Messages, System.SysUtils, System.Variants, System.Classes,
  Vcl.Graphics, Vcl.Controls, Vcl.Forms, Vcl.Dialogs, SniffProcessor,
  Vcl.StdCtrls, Vcl.ExtCtrls, Vcl.Mask;

type
  TFrameUniversalTweaker = class(TFrame)
    StaticText1: TStaticText;
    edPath: TLabeledEdit;
    edValue: TLabeledEdit;
    Label1: TLabel;
    chkOldValueCheck: TCheckBox;
    cmbOldValueMode: TComboBox;
    edOldValue: TEdit;
    cmbNewValueMode: TComboBox;
    edBlocks: TEdit;
    chkInherited: TCheckBox;
    Button1: TButton;
    chkReport: TCheckBox;
    procedure chkOldValueCheckClick(Sender: TObject);
    procedure Button1Click(Sender: TObject);
  private
    { Private declarations }
  public
    { Public declarations }
  end;

  TTweakOldValueMode = (ovmEqual = 0, ovmNotEqual, ovmGreater, ovmLesser,
    ovmContains, ovmDoesntContain, ovmStartsWith, ovmEndsWith,
    ovmAnd, ovmNotAnd);

  TTweakNewValueMode = (nvmSet = 0, nvmAdd, nvmMul, nvmReplace, nvmPrepend, nvmAppend);

  TProcUniversalTweaker = class(TProcBase)
  private
    Frame: TFrameUniversalTweaker;
    fBlocks: array of string;
    fInherited: Boolean;
    fPath: string;
    fValue: string;
    fValueMode: TTweakNewValueMode;
    fOldValueCheck: Boolean;
    fOldValueMode: TTweakOldValueMode;
    fOldValue: string;
    fReportOnly: Boolean;
  public
    constructor Create(aManager: TProcManager); override;
    function GetFrame(aOwner: TComponent): TFrame; override;
    procedure OnShow; override;
    procedure OnHide; override;
    procedure OnStart; override;

    function ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes; override;
  end;

implementation

{$R *.dfm}

uses
  System.StrUtils,
  System.Math,
  wbDataFormat,
  wbDataFormatNif,
  wbDataFormatMaterial;

constructor TProcUniversalTweaker.Create(aManager: TProcManager);
begin
  inherited;

  fTitle := 'Universal tweaker';
  fSupportedGames := [gtTES3, gtTES4, gtFO3, gtFNV, gtTES5, gtSSE, gtFO4];
  fExtensions := ['nif', 'kf', 'bgsm', 'bgem'];
end;

function TProcUniversalTweaker.GetFrame(aOwner: TComponent): TFrame;
begin
  Frame := TFrameUniversalTweaker.Create(aOwner);
  Result := Frame;
end;

procedure TFrameUniversalTweaker.Button1Click(Sender: TObject);
const
  sText = 'Convert NIF to JSON to see how fields are named and their values.'#13#13 +
    'Divide float value field by 3:'#13 +
    'Mul "0.333"'#13#13 +
    'Text replacement in string fields:'#13 +
    'Replace with "Rock" if old value Contains "Stone"'#13#13 +
    'Removing prefix "Ext" from string fields if present:'#13 +
    'Replace with "" if old value Starts with "Ext"'#13#13 +
    'Adding bit 3 to the integer flags field:'#13 +
    'Add "8" if old value NOT AND & "8"'#13#13 +
    'Removing bit 2 from the integer flags field:'#13 +
    'Add "-4" if old value AND & "4"'#13#13 +
    'Adding flag to the field with named flags:'#13 +
    'Append " | Environment_Mapping" if old value Doesn''t Contain "Environment_Mapping"'#13#13 +
    'Removing flag from the field with named flags:'#13 +
    'Replace with "" if old value Contains "Environment_Mapping"'#13#13;
begin
  with TTaskDialog.Create(Self) do try
    Text := sText;
    Caption := Application.Title;
    Flags := [tfUseHiconMain];
    CustomMainIcon := Application.Icon;
    CommonButtons := [tcbClose];
    Execute;
  finally
    Free;
  end;
end;

procedure TFrameUniversalTweaker.chkOldValueCheckClick(Sender: TObject);
begin
  cmbOldValueMode.Enabled := chkOldValueCheck.Checked;
  edOldValue.Enabled := chkOldValueCheck.Checked;
end;

procedure TProcUniversalTweaker.OnShow;
begin
  Frame.chkReport.Checked := StorageGetBool('bReportOnly', Frame.chkReport.Checked);
  Frame.edBlocks.Text := StorageGetString('sBlocks', Frame.edBlocks.Text);
  Frame.chkInherited.Checked := StorageGetBool('bDescendants', Frame.chkInherited.Checked);
  Frame.edPath.Text := StorageGetString('sPath', Frame.edPath.Text);
  Frame.cmbNewValueMode.ItemIndex := StorageGetInteger('iValueMode', Frame.cmbNewValueMode.ItemIndex);
  Frame.edValue.Text := StorageGetString('sValue', Frame.edValue.Text);
  Frame.chkOldValueCheck.Checked := StorageGetBool('bOldValueCheck', Frame.chkOldValueCheck.Checked);
  Frame.cmbOldValueMode.ItemIndex := StorageGetInteger('iOldValueMode', Frame.cmbOldValueMode.ItemIndex);
  Frame.edOldValue.Text := StorageGetString('sOldValue', Frame.edOldValue.Text);
  Frame.chkOldValueCheckClick(nil);
end;

procedure TProcUniversalTweaker.OnHide;
begin
  StorageSetString('sBlocks', Frame.edBlocks.Text);
  StorageSetBool('bDescendants', Frame.chkInherited.Checked);
  StorageSetString('sPath', Frame.edPath.Text);
  StorageSetInteger('iValueMode', Frame.cmbNewValueMode.ItemIndex);
  StorageSetString('sValue', Frame.edValue.Text);
  StorageSetBool('bOldValueCheck', Frame.chkOldValueCheck.Checked);
  StorageSetInteger('iOldValueMode', Frame.cmbOldValueMode.ItemIndex);
  StorageSetString('sOldValue', Frame.edOldValue.Text);
  StorageSetBool('bReportOnly', Frame.chkReport.Checked);
end;

procedure TProcUniversalTweaker.OnStart;
begin
  with TStringList.Create do try
    Delimiter := ',';
    StrictDelimiter := True;
    DelimitedText := Frame.edBlocks.Text;
    SetLength(fBlocks, Count);
    for var i: Integer := 0 to Pred(Count) do
      fBlocks[i] := Trim(Strings[i]);
  finally
    Free;
  end;

  fInherited := Frame.chkInherited.Checked;

  fPath := Frame.edPath.Text;
  if fPath = '' then
    raise Exception.Create('Element path can not be empty');

  fValueMode := TTweakNewValueMode(Frame.cmbNewValueMode.ItemIndex);
  fValue := Frame.edValue.Text;

  fOldValueCheck := Frame.chkOldValueCheck.Checked;
  fOldValueMode := TTweakOldValueMode(Frame.cmbOldValueMode.ItemIndex);
  fOldValue := Frame.edOldValue.Text;

  if (fValueMode = nvmReplace) and (not fOldValueCheck or not (fOldValueMode in [ovmContains, ovmStartsWith, ovmEndsWith]) or (fOldValue = '')) then
    raise Exception.Create('When replacing, old value must be checked and "Contains", "Start with" or "Ends with" non-empty value');

  if fValueMode in [nvmAdd, nvmMul] then try
    dfStrToFloat(fValue);
  except
    raise Exception.Create('Value must be a number');
  end;

  if fOldValueCheck and (fOldValueMode in [ovmGreater, ovmLesser]) then try
    dfStrToFloat(fOldValue);
  except
    raise Exception.Create('Old value must be a number');
  end;

  fReportOnly := Frame.chkReport.Checked;
end;


function ModifyElement(aBlock: TdfElement; const aPath, aValue, aOldValue: string;
  aValueMode: TTweakNewValueMode;
  aOldValueCheck: Boolean;
  aOldValueMode: TTweakOldValueMode;
  Log: TStrings
): Boolean;
var
  CurrentValue, NewValue: string;
  FloatCurrentValue, FloatOldValue, FloatNewValue: Extended;
  Matched: Boolean;

  function ToFloat: Boolean;
  begin
    Result := True;
    try
      if aOldValueCheck then
        FloatOldValue := dfStrToFloat(aOldValue);
      FloatCurrentValue := dfStrToFloat(CurrentValue);
      FloatNewValue := dfStrToFloat(aValue);
    except
      Result := False;
    end;
  end;

begin
  CurrentValue := aBlock.EditValues[aPath];
  Result := False;
  Matched := True;

  if aOldValueCheck then
  case aOldValueMode of
    ovmEqual:    Matched := (ToFloat and SameValue(FloatCurrentValue, FloatOldValue)) or SameText(CurrentValue, aOldValue);
    ovmNotEqual: Matched := (ToFloat and not SameValue(FloatCurrentValue, FloatOldValue)) or not SameText(CurrentValue, aOldValue);
    ovmGreater:  Matched := ToFloat and (FloatCurrentValue > FloatOldValue);
    ovmLesser:   Matched := ToFloat and (FloatCurrentValue < FloatOldValue);
    ovmContains: Matched := ContainsText(CurrentValue, aOldValue);
    ovmDoesntContain: Matched := not ContainsText(CurrentValue, aOldValue);
    ovmStartsWith: Matched := CurrentValue.StartsWith(aOldValue, True);
    ovmEndsWith: Matched := CurrentValue.EndsWith(aOldValue, True);
    ovmAnd:      Matched := ToFloat and (Trunc(FloatCurrentValue) and Trunc(FloatOldValue) = Trunc(FloatOldValue));
    ovmNotAnd:   Matched := ToFloat and (Trunc(FloatCurrentValue) and Trunc(FloatOldValue) = 0);
  end;

  if not Matched then
    Exit;

  case aValueMode of
    nvmSet:     NewValue := aValue;
    nvmAdd:     if ToFloat then NewValue := dfFloatToStr(FloatCurrentValue + FloatNewValue);
    nvmMul:     if ToFloat then NewValue := dfFloatToStr(FloatCurrentValue * FloatNewValue);
    nvmReplace: case aOldValueMode of
        ovmContains:   NewValue := StringReplace(CurrentValue, aOldValue, aValue, [rfReplaceAll, rfIgnoreCase]);
        ovmStartsWith: NewValue := aValue + Copy(CurrentValue, Length(aOldValue) + 1, Length(CurrentValue));
        ovmEndsWith:   NewValue := Copy(CurrentValue, 1, Length(CurrentValue) - Length(aOldValue)) + aValue;
      end;
    nvmPrepend: NewValue := aValue + CurrentValue;
    nvmAppend : NewValue := CurrentValue + aValue;
  end;

  // for Add and Mul if fractional part is zero (ends with .00000)
  // then remove it in case we are working with int field
  if aValueMode in [nvmAdd, nvmMul] then begin
    var z := Copy(dfFloatToStr(1), 2, 100);
    if NewValue.EndsWith(z) then
      NewValue := Copy(NewValue, 1, Length(NewValue) - Length(z));
  end;

  aBlock.EditValues[aPath] := NewValue;
  Result := CurrentValue <> aBlock.EditValues[aPath];
  if Assigned(Log) and Result then
    Log.Add(#9 + aBlock.Name + '\' + aPath + ': Changed from "' + CurrentValue + '" to "' + aBlock.EditValues[aPath] + '"');
end;

function TProcUniversalTweaker.ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes;
var
  nif: TwbNifFile;
  BGSM: TwbBGSMFile;
  BGEM: TwbBGEMFile;
  Log: TStringList;
  i: Integer;
  block: TwbNifBlock;
  bChanged, bMatched: Boolean;
  ext: string;
begin
  bChanged := False;
  nif := nil; BGSM := nil; BGEM := nil; Log := nil; // suppress compiler warning

  if fReportOnly then
    Log := TStringList.Create;

  ext := ExtractFileExt(aFileName);
  try
    // *.NIF file
    if SameText(ext, '.nif') or SameText(ext, '.kf') then begin
      nif := TwbNifFile.Create;
      nif.LoadFromFile(aInputDirectory + aFileName);

      // processing specific block by path
      if (Length(fBlocks) = 1) and (Pos('\', fBlocks[0]) <> 0) then begin
        block := nif.BlockByPath(fBlocks[0]);
        if not Assigned(block) then
          Exit;

        bChanged := ModifyElement(block, fPath, fValue, fOldValue, fValueMode, fOldValueCheck, fOldValueMode, Log);
      end

      else begin
        // if processing BSXFlags and it is missing, then add it
        if (nif.NifVersion >= nfTES4) and (fBlocks[0] = 'BSXFlags') then
          if not Assigned(nif.BlockByType('BSXFlags')) and (nif.BlocksCount <> 0) and nif.RootNode.IsNiObject('NiNode') then
            nif.RootNode.AddExtraData('BSXFlags').EditValues['Name'] := 'BSX';

        // processing blocks by type
        for i := 0 to Pred(nif.BlocksCount) do begin
          block := nif.Blocks[i];

          bMatched:= False;
          for var s: string in fBlocks do
            if block.IsNiObject(s, fInherited) then
              bMatched := True;

          if not bMatched and (Length(fBlocks) <> 0) then
            Continue;

          bChanged := ModifyElement(block, fPath, fValue, fOldValue, fValueMode, fOldValueCheck, fOldValueMode, Log) or bChanged;
        end;
      end;
    end

    // *.BGSM file
    else if SameText(ext, '.bgsm') then begin
      BGSM := TwbBGSMFile.Create;
      BGSM.LoadFromFile(aInputDirectory + aFileName);

      bChanged := ModifyElement(BGSM, fPath, fValue, fOldValue, fValueMode, fOldValueCheck, fOldValueMode, Log) or bChanged;
    end

    // *.BGEM file
    else if SameText(ext, '.bgem') then begin
      BGEM := TwbBGEMFile.Create;
      BGEM.LoadFromFile(aInputDirectory + aFileName);

      bChanged := ModifyElement(BGEM, fPath, fValue, fOldValue, fValueMode, fOldValueCheck, fOldValueMode, Log) or bChanged;
    end;

    if bChanged and not fReportOnly then begin
      if Assigned(nif) then nif.SaveToData(Result);
      if Assigned(BGSM) then BGSM.SaveToData(Result);
      if Assigned(BGEM) then BGEM.SaveToData(Result);
    end;

    if bChanged and fReportOnly then begin
      fManager.AddMessage(aFileName);
      fManager.AddMessages(Log);
      fManager.AddMessage('');
    end;

  finally
    if Assigned(nif) then nif.Free;
    if Assigned(BGSM) then BGSM.Free;
    if Assigned(BGEM) then BGEM.Free;
    if Assigned(Log) then Log.Free;
  end;

end;

end.
