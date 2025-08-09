{******************************************************************************

  This Source Code Form is subject to the terms of the Mozilla Public License,
  v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain
  one at https://mozilla.org/MPL/2.0/.

*******************************************************************************}

unit ProcConvertShader30;

interface

uses
  Winapi.Windows, Winapi.Messages, System.SysUtils, System.Variants, System.Classes,
  Vcl.Graphics, Vcl.Controls, Vcl.Forms, Vcl.Dialogs, Vcl.StdCtrls, SniffProcessor;

type
  TFrameConvertShader30 = class(TFrame)
    StaticText1: TStaticText;
  private
    { Private declarations }
  public
    { Public declarations }
  end;

  TProcConvertShader30 = class(TProcBase)
  private
    Frame: TFrameConvertShader30;
  public
    constructor Create(aManager: TProcManager); override;
    function GetFrame(aOwner: TComponent): TFrame; override;

    function ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes; override;
  end;


implementation

{$R *.dfm}

uses
  wbDataFormat,
  wbDataFormatNif;

constructor TProcConvertShader30.Create(aManager: TProcManager);
begin
  inherited;

  fTitle := 'Convert to shader 3.0';
  fSupportedGames := [gtFO3, gtFNV];
  fExtensions := ['nif'];
end;

function TProcConvertShader30.GetFrame(aOwner: TComponent): TFrame;
begin
  Frame := TFrameConvertShader30.Create(aOwner);
  Result := Frame;
end;

function TProcConvertShader30.ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes;
var
  nif: TwbNifFile;
  Block: TwbNifBlock;
  bChanged: Boolean;
  i: Integer;
begin
  nif := TwbNifFile.Create;
  bChanged := False;

  try
    nif.LoadFromFile(aInputDirectory + aFileName);

    for i := 0 to Pred(nif.BlocksCount) do begin
      block := nif.Blocks[i];

      if block.BlockType <> 'BSShaderPPLightingProperty' then
        Continue;

      nif.ConvertBlock(i, 'Lighting30ShaderProperty');
      block := nif.Blocks[i];
      block.Elements['Shader Type'].EditValue := 'SHADER_LIGHTING30';

      bChanged := True;
    end;

    if bChanged then
      nif.SaveToData(Result);

  finally
    nif.Free;
  end;
end;



end.
