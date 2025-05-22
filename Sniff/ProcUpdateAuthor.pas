{******************************************************************************

  This Source Code Form is subject to the terms of the Mozilla Public License,
  v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain
  one at https://mozilla.org/MPL/2.0/.

*******************************************************************************}

unit ProcUpdateAuthor;

interface

uses
  Winapi.Windows, Winapi.Messages, System.SysUtils, System.Variants, System.Classes,
  Vcl.Graphics, Vcl.Controls, Vcl.Forms, Vcl.Dialogs, Vcl.StdCtrls, Vcl.Mask,
  Vcl.ExtCtrls, SniffProcessor;

type
  TFrameUpdateAuthor = class(TFrame)
    StaticText1: TStaticText;
    edAuthor: TLabeledEdit;
    edProcessScript: TLabeledEdit;
    edExportScript: TLabeledEdit;
  private
    { Private declarations }
  public
    { Public declarations }
  end;

  TProcUpdateAuthor = class(TProcBase)
  private
    Frame: TFrameUpdateAuthor;
    fAuthor: string;
    fProcessScript: string;
    fExportScript: string;
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
  wbDataFormat,
  wbDataFormatNif;

constructor TProcUpdateAuthor.Create(aManager: TProcManager);
begin
  inherited;

  fTitle := 'Update author';
  fSupportedGames := [gtTES4, gtFO3, gtFNV, gtTES5, gtSSE, gtFO4];
  fExtensions := ['nif', 'kf'];
end;

function TProcUpdateAuthor.GetFrame(aOwner: TComponent): TFrame;
begin
  Frame := TFrameUpdateAuthor.Create(aOwner);
  Result := Frame;
end;

procedure TProcUpdateAuthor.OnShow;
begin
  Frame.edAuthor.Text := StorageGetString('sAuthor', Frame.edAuthor.Text);
  Frame.edProcessScript.Text := StorageGetString('sProcessScript', Frame.edProcessScript.Text);
  Frame.edExportScript.Text := StorageGetString('sExportScript', Frame.edExportScript.Text);
end;

procedure TProcUpdateAuthor.OnHide;
begin
  StorageSetString('sAuthor', Frame.edAuthor.Text);
  StorageSetString('sProcessScript', Frame.edProcessScript.Text);
  StorageSetString('sExportScript', Frame.edExportScript.Text);
end;

procedure TProcUpdateAuthor.OnStart;
begin
  fAuthor := Frame.edAuthor.Text;
  fProcessScript := Frame.edProcessScript.Text;
  fExportScript := Frame.edExportScript.Text;

  if (fAuthor = '*') and (fProcessScript = '*') and (fExportScript = '*') then
    raise Exception.Create('Nothing to update');
end;

function TProcUpdateAuthor.ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes;
var
  nif: TwbNifFile;
  bChanged: Boolean;
begin
  bChanged := False;
  nif := TwbNifFile.Create;
  nif.Options := [nfoRemoveUnusedStrings];
  try
    nif.LoadFromFile(aInputDirectory + aFileName);

    var info := nif.Header.Elements['Export Info'];
    if not Assigned(info) then
      Exit;

    if (fAuthor <> '*') and (info.EditValues['Author'] <> fAuthor) then begin
      info.EditValues['Author'] := fAuthor;
      bChanged := True;
    end;

    if (fProcessScript <> '*') and (info.EditValues['Process Script'] <> fProcessScript) then begin
      info.EditValues['Process Script'] := fProcessScript;
      bChanged := True;
    end;

    if (fExportScript <> '*') and (info.EditValues['Export Script'] <> fExportScript) then begin
      info.EditValues['Export Script'] := fExportScript;
      bChanged := True;
    end;

    if bChanged then
      nif.SaveToData(Result);

  finally
    nif.Free;
  end;

end;

end.
