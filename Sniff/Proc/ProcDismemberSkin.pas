unit ProcDismemberSkin;

interface

uses
  Winapi.Windows, Winapi.Messages, System.SysUtils, System.Variants, System.Classes,
  Vcl.Graphics, Vcl.Controls, Vcl.Forms, Vcl.Dialogs, Vcl.StdCtrls, SniffProcessor;

type
  TFrameDismemberSkin = class(TFrame)
    StaticText1: TStaticText;
  private
    { Private declarations }
  public
    { Public declarations }
  end;

  TProcDismemberSkin = class(TProcBase)
  private
    Frame: TFrameDismemberSkin;
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

constructor TProcDismemberSkin.Create(aManager: TProcManager);
begin
  inherited;

  fTitle := 'Dismember NiSkinInstance';
  fSupportedGames := [gtFO3, gtFNV, gtTES5, gtSSE];
  fExtensions := ['nif'];
end;

function TProcDismemberSkin.GetFrame(aOwner: TComponent): TFrame;
begin
  Frame := TFrameDismemberSkin.Create(aOwner);
  Result := Frame;
end;

function TProcDismemberSkin.ProcessFile(const aInputDirectory, aOutputDirectory: string; var aFileName: string): TBytes;
var
  nif: TwbNifFile;
  Block, skinpart: TwbNifBlock;
  bChanged: Boolean;
  i, j, parts: Integer;
begin
  nif := TwbNifFile.Create;
  nif.Options := [nfoCollapseLinkArrays];
  bChanged := False;

  try
    nif.LoadFromFile(aInputDirectory + aFileName);

    for i := 0 to Pred(nif.BlocksCount) do begin
      block := nif.Blocks[i];

      if block.BlockType <> 'NiSkinInstance' then
        Continue;

      nif.ConvertBlock(i, 'BSDismemberSkinInstance');
      block := nif.Blocks[i];

      skinpart := TwbNifBlock(block.Elements['Skin Partition'].LinksTo);

      if Assigned(skinpart) then
        parts := skinpart.NativeValues['Num Partitions']
      else
        parts := 1;

      for j := 0 to Pred(parts) do
        block.Elements['Partitions'].Add;

      bChanged := True;
    end;

    if bChanged then
      nif.SaveToData(Result);

  finally
    nif.Free;
  end;
end;

end.
