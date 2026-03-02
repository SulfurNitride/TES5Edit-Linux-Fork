unit xeMainFormLaz;

{$mode objfpc}{$H+}

interface

uses
  Classes,
  SysUtils,
  Forms,
  Controls,
  Graphics,
  Dialogs,
  Menus,
  ComCtrls,
  StdCtrls,
  ExtCtrls,
  Buttons,
  LCLType,
  xedit_ffi;

const
  APP_VERSION = '4.1.6';

function SignatureToFriendlyName(const ASig: string): string;

const
  { Maximum number of search results returned by editor ID search }
  MAX_EDITOR_ID_RESULTS = 256;

type
  { Node types for navigation tree Data pointers }
  TNodeType = (ntPlugin, ntGroup, ntRecord);

  { Data record attached to each navigation tree node via Data pointer.
    Allocated on heap with New(), freed in vstNavDeletion. }
  PNodeData = ^TNodeData;
  TNodeData = record
    NodeType: TNodeType;
    PluginIndex: Integer;
    GroupIndex: Integer;
    RecordIndex: Integer;
    Loaded: Boolean;       { True if child nodes have been populated (groups only) }
    RecordCount: Integer;  { Cached record count for group nodes }
  end;

  TfrmMain = class(TForm)
    { --- Main Menu --- }
    MainMenu1: TMainMenu;

    { File menu }
    mniFile: TMenuItem;
    mniFileOpen: TMenuItem;
    mniFileMO2Open: TMenuItem;
    mniFileSep1: TMenuItem;
    mniFileExit: TMenuItem;

    { Navigation menu }
    mniNavigation: TMenuItem;
    mniNavCompareSelected: TMenuItem;
    mniNavFilterApply: TMenuItem;
    mniNavFilterRemove: TMenuItem;
    mniNavSep1: TMenuItem;
    mniNavCheckForErrors: TMenuItem;
    mniNavApplyScript: TMenuItem;
    mniNavSep2: TMenuItem;
    mniNavOptions: TMenuItem;

    { View menu }
    mniView: TMenuItem;
    mniViewEdit: TMenuItem;
    mniViewAdd: TMenuItem;
    mniViewRemove: TMenuItem;
    mniViewSep1: TMenuItem;
    mniViewCopyToSelectedRecords: TMenuItem;
    mniViewColumnWidths: TMenuItem;

    { Referenced By menu }
    mniReferencedBy: TMenuItem;
    mniRefByCompareSelected: TMenuItem;
    mniRefByApplyScript: TMenuItem;
    mniRefByCopyOverrideInto: TMenuItem;
    mniRefByRemove: TMenuItem;

    { Messages menu }
    mniMessages: TMenuItem;
    mniMessagesClear: TMenuItem;
    mniMessagesSaveSelected: TMenuItem;
    mniMessagesAutoscroll: TMenuItem;

    { Main menu }
    mniMain: TMenuItem;
    mniMainLocalization: TMenuItem;
    mniMainPluggyLink: TMenuItem;
    mniMainSep1: TMenuItem;
    mniMainSave: TMenuItem;
    mniMainOptions: TMenuItem;

    { Help menu }
    mniHelp: TMenuItem;
    mniHelpAbout: TMenuItem;

    { --- Toolbar --- }
    ToolBar1: TToolBar;
    bnBack: TToolButton;
    bnForward: TToolButton;
    tbSep1: TToolButton;
    bnSave: TToolButton;
    tbSep2: TToolButton;
    lblGame: TLabel;
    cobGame: TComboBox;

    { --- Open dialog --- }
    dlgOpenPlugin: TOpenDialog;
    dlgSelectMO2Dir: TSelectDirectoryDialog;

    { --- Left navigation panel --- }
    pnlNav: TPanel;

    { Search bar at top of nav }
    pnlSearch: TPanel;
    lblFormID: TLabel;
    edFormIDSearch: TEdit;
    lblEditorID: TLabel;
    edEditorIDSearch: TEdit;

    { Filter hint label }
    lblFilterHint: TLabel;

    { Navigation tree (placeholder for VirtualTreeView-Lazarus) }
    vstNav: TTreeView;

    { Bottom filter bar }
    pnlNavBottom: TPanel;
    lblFileNameFilter: TLabel;
    edFileNameFilter: TEdit;

    { --- Splitter between nav and right panel --- }
    splElements: TSplitter;

    { --- Right panel --- }
    pnlRight: TPanel;
    pgMain: TPageControl;

    { View tab }
    tbsView: TTabSheet;
    pnlViewTop: TPanel;
    lblViewFilterName: TLabel;
    edViewFilterName: TEdit;
    cobViewFilter: TComboBox;
    lblViewFilterValue: TLabel;
    edViewFilterValue: TEdit;
    bnLegend: TSpeedButton;
    vstView: TTreeView;

    { Referenced By tab }
    tbsReferencedBy: TTabSheet;
    lvReferencedBy: TListView;

    { Messages tab }
    tbsMessages: TTabSheet;
    mmoMessages: TMemo;

    { Information tab }
    tbsInfo: TTabSheet;
    mmoInfo: TMemo;

    { --- Status Bar --- }
    stbMain: TStatusBar;

    { --- Cancel panel (hidden, for long operations) --- }
    pnlCancel: TPanel;
    btnCancel: TButton;

    { --- Navigation context popup menu --- }
    pmNav: TPopupMenu;
    mniNavCtxQuickClean: TMenuItem;
    mniNavCtxSep1: TMenuItem;
    mniNavCtxCheckITM: TMenuItem;
    mniNavCtxRemoveITM: TMenuItem;
    mniNavCtxUndeleteRefs: TMenuItem;

    { Navigation menu - Clean submenu }
    mniNavSep3: TMenuItem;
    mniNavClean: TMenuItem;
    mniNavCleanQuick: TMenuItem;
    mniNavCleanSep1: TMenuItem;
    mniNavCleanCheckITM: TMenuItem;
    mniNavCleanRemoveITM: TMenuItem;
    mniNavCleanUndeleteRefs: TMenuItem;

    { Timer for async refby build polling }
    tmrRefByBuild: TTimer;

    { Event handlers }
    procedure FormCreate(Sender: TObject);
    procedure FormDestroy(Sender: TObject);
    procedure FormCloseQuery(Sender: TObject; var CanClose: Boolean);
    procedure mniFileOpenClick(Sender: TObject);
    procedure mniFileMO2OpenClick(Sender: TObject);
    procedure mniFileExitClick(Sender: TObject);
    procedure mniHelpAboutClick(Sender: TObject);
    procedure cobGameChange(Sender: TObject);
    procedure DoQuickClean(Sender: TObject);
    procedure DoCheckITM(Sender: TObject);
    procedure DoRemoveITM(Sender: TObject);
    procedure DoUndeleteRefs(Sender: TObject);
    { T19: Lazy-loading, node data, and search event handlers }
    procedure vstNavExpanding(Sender: TObject; Node: TTreeNode;
      var AllowExpansion: Boolean);
    procedure vstNavDeletion(Sender: TObject; Node: TTreeNode);
    procedure vstNavChange(Sender: TObject; Node: TTreeNode);
    procedure edFormIDSearchKeyPress(Sender: TObject; var Key: Char);
    procedure edEditorIDSearchKeyPress(Sender: TObject; var Key: Char);
    procedure vstNavDragOver(Sender, Source: TObject; X, Y: Integer;
      State: TDragState; var Accept: Boolean);
    procedure vstNavDragDrop(Sender, Source: TObject; X, Y: Integer);
    procedure tmrRefByBuildTimer(Sender: TObject);
  private
    FEngineReady: Boolean;
    FEngineHandle: Pointer;
    FCurrentGame: string;
    FMO2Active: Boolean;
    FMO2Profile: string;
    { Track currently displayed record to avoid redundant reloads }
    FViewPluginIdx: Integer;
    FViewGroupIdx: Integer;
    FViewRecordIdx: Integer;
    procedure SetStatusText(const AText: string);
    procedure LogMessage(const AMsg: string);
    procedure InitEngine;
    procedure ShutdownEngine;
    procedure LoadPluginFile(const AFilePath: string);
    procedure PopulateNavTree(APluginIndex: Integer; const AFileName: string);
    function GetSelectedGameName: string;
    function GetGameFilterExtensions: string;
    function GetSelectedPluginIndex(out PluginName: string): Integer;
    procedure LoadMO2Folder(const APath: string);
    function ShowMO2ProfileSelector(AProfileCount: Integer): string;
    procedure PopulateNavFromLoadOrder;
    { T19: Node data allocation }
    function AllocNodeData(AType: TNodeType; APlugin, AGroup,
      ARecord: Integer): PNodeData;
    { T19: Lazy-load records for a group node }
    procedure LoadGroupRecords(ANode: TTreeNode; AData: PNodeData);
    { T19: Search helpers }
    procedure DoFormIDSearch;
    procedure DoEditorIDSearch;
    function FindGroupNode(APluginIdx, AGroupIdx: Integer): TTreeNode;
    function FindRecordNode(AGroupNode: TTreeNode;
      ARecordIdx: Integer): TTreeNode;
    procedure EnsureGroupExpanded(APluginIdx, AGroupIdx: Integer;
      out AGroupNode: TTreeNode);
    procedure SelectAndFocusNode(ANode: TTreeNode);
    { Record view }
    procedure PopulateRecordView(APluginIdx, AGroupIdx, ARecordIdx: Integer);
    procedure UpdateRecordInfo(APluginIdx, AGroupIdx, ARecordIdx: Integer);
    procedure ClearRecordView;
    { Referenced By tab }
    procedure PopulateReferencedBy(APluginIdx, AGroupIdx, ARecordIdx: Integer);
    function BytesToHex(ABuf: PChar; ALen: Integer): string;
    function TryDecodeString(ABuf: PChar; ALen: Integer): string;
    function IsTextSubrecord(const ASig: string): Boolean;
  public
  end;

var
  frmMain: TfrmMain;

implementation

{$R *.lfm}

function SignatureToFriendlyName(const ASig: string): string;
begin
  case ASig of
    'AACT': Result := 'Action';
    'ACHR': Result := 'Placed NPC';
    'ACTI': Result := 'Activator';
    'ADDN': Result := 'Addon Node';
    'ALCH': Result := 'Ingestible';
    'AMMO': Result := 'Ammunition';
    'ANIO': Result := 'Animated Object';
    'APPA': Result := 'Apparatus';
    'ARMA': Result := 'Armor Addon';
    'ARMO': Result := 'Armor';
    'ARTO': Result := 'Art Object';
    'ASPC': Result := 'Acoustic Space';
    'ASTP': Result := 'Association Type';
    'AVIF': Result := 'Actor Value Information';
    'BOOK': Result := 'Book';
    'BPTD': Result := 'Body Part Data';
    'CAMS': Result := 'Camera Shot';
    'CELL': Result := 'Cell';
    'CLAS': Result := 'Class';
    'CLFM': Result := 'Color';
    'CLMT': Result := 'Climate';
    'COBJ': Result := 'Constructible Object';
    'COLL': Result := 'Collision Layer';
    'CONT': Result := 'Container';
    'CPTH': Result := 'Camera Path';
    'CSTY': Result := 'Combat Style';
    'DEBR': Result := 'Debris';
    'DIAL': Result := 'Dialog Topic';
    'DLBR': Result := 'Dialog Branch';
    'DLVW': Result := 'Dialog View';
    'DOBJ': Result := 'Default Object Manager';
    'DOOR': Result := 'Door';
    'DUAL': Result := 'Dual Cast Data';
    'ECZN': Result := 'Encounter Zone';
    'EFSH': Result := 'Effect Shader';
    'ENCH': Result := 'Object Effect';
    'EQUP': Result := 'Equip Type';
    'EXPL': Result := 'Explosion';
    'EYES': Result := 'Eyes';
    'FACT': Result := 'Faction';
    'FLOR': Result := 'Flora';
    'FLST': Result := 'FormID List';
    'FSTP': Result := 'Footstep';
    'FSTS': Result := 'Footstep Set';
    'FURN': Result := 'Furniture';
    'GLOB': Result := 'Global';
    'GMST': Result := 'Game Setting';
    'GRAS': Result := 'Grass';
    'HAIR': Result := 'Hair';
    'HAZD': Result := 'Hazard';
    'HDPT': Result := 'Head Part';
    'IDLE': Result := 'Idle Animation';
    'IDLM': Result := 'Idle Marker';
    'IMAD': Result := 'Image Space Adapter';
    'IMGS': Result := 'Image Space';
    'INFO': Result := 'Dialog Response';
    'INGR': Result := 'Ingredient';
    'IPCT': Result := 'Impact';
    'IPDS': Result := 'Impact Data Set';
    'KEYM': Result := 'Key';
    'KYWD': Result := 'Keyword';
    'LAND': Result := 'Landscape';
    'LCRT': Result := 'Location Reference Type';
    'LCTN': Result := 'Location';
    'LGTM': Result := 'Lighting Template';
    'LIGH': Result := 'Light';
    'LSCR': Result := 'Load Screen';
    'LTEX': Result := 'Landscape Texture';
    'LVLI': Result := 'Leveled Item';
    'LVLN': Result := 'Leveled NPC';
    'LVSP': Result := 'Leveled Spell';
    'MATO': Result := 'Material Object';
    'MATT': Result := 'Material Type';
    'MESG': Result := 'Message';
    'MGEF': Result := 'Magic Effect';
    'MISC': Result := 'Misc. Item';
    'MOVT': Result := 'Movement Type';
    'MSTT': Result := 'Moveable Static';
    'MUSC': Result := 'Music Type';
    'MUST': Result := 'Music Track';
    'NAVI': Result := 'Navigation Mesh Info Map';
    'NAVM': Result := 'Navigation Mesh';
    'NOTE': Result := 'Note';
    'NPC_': Result := 'Non-Player Character (Actor)';
    'OTFT': Result := 'Outfit';
    'PACK': Result := 'Package';
    'PERK': Result := 'Perk';
    'PGRE': Result := 'Placed Grenade';
    'PHZD': Result := 'Placed Hazard';
    'PROJ': Result := 'Projectile';
    'QUST': Result := 'Quest';
    'RACE': Result := 'Race';
    'REFR': Result := 'Placed Object';
    'REGN': Result := 'Region';
    'RELA': Result := 'Relationship';
    'REVB': Result := 'Reverb Parameters';
    'RFCT': Result := 'Visual Effect';
    'SCEN': Result := 'Scene';
    'SCPT': Result := 'Script';
    'SCRL': Result := 'Scroll';
    'SHOU': Result := 'Shout';
    'SLGM': Result := 'Soul Gem';
    'SMBN': Result := 'Story Manager Branch Node';
    'SMEN': Result := 'Story Manager Event Node';
    'SMQN': Result := 'Story Manager Quest Node';
    'SNCT': Result := 'Sound Category';
    'SNDR': Result := 'Sound Descriptor';
    'SOPM': Result := 'Sound Output Model';
    'SOUN': Result := 'Sound Marker';
    'SPEL': Result := 'Spell';
    'SPGD': Result := 'Shader Particle Geometry';
    'STAT': Result := 'Static';
    'TACT': Result := 'Talking Activator';
    'TREE': Result := 'Tree';
    'TXST': Result := 'Texture Set';
    'VTYP': Result := 'Voice Type';
    'WATR': Result := 'Water';
    'WEAP': Result := 'Weapon';
    'WOOP': Result := 'Word of Power';
    'WRLD': Result := 'Worldspace';
    'WTHR': Result := 'Weather';
    'AECH': Result := 'Audio Effect Chain';
    'AMDL': Result := 'Aim Model';
    'BNDS': Result := 'Bendable Spline';
    'DFOB': Result := 'Default Object';
    'DMGT': Result := 'Damage Type';
    'GDRY': Result := 'God Rays';
    'INNR': Result := 'Instance Naming Rules';
    'KSSM': Result := 'Sound Keyword Mapping';
    'LAYR': Result := 'Layer';
    'LENS': Result := 'Lens Flare';
    'NOCM': Result := 'Navigation Mesh Obstacle Manager';
    'OMOD': Result := 'Object Modification';
    'PKIN': Result := 'Pack-In';
    'RFGP': Result := 'Reference Group';
    'SCCO': Result := 'Scene Collection';
    'TERM': Result := 'Terminal';
    'TRNS': Result := 'Transform';
    'ZOOM': Result := 'Zoom';
  else
    Result := ASig;
  end;
end;

{ --------------------------------------------------------------------------- }
{ Form lifecycle                                                                }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.FormCreate(Sender: TObject);
begin
  FEngineReady := False;
  FEngineHandle := nil;
  FCurrentGame := '';
  FMO2Active := False;
  FMO2Profile := '';
  FViewPluginIdx := -1;
  FViewGroupIdx := -1;
  FViewRecordIdx := -1;

  { Default game selection }
  cobGame.ItemIndex := 0;

  { Disable hints to avoid Qt6/Wayland transientParent popup warnings }
  Application.ShowHint := False;

  SetStatusText('Ready - Select a game and open a plugin file');
  LogMessage('xEdit Lazarus Port v' + APP_VERSION + ' started.');
  LogMessage('Select a game from the toolbar, then use File > Open to load a plugin.');
end;

procedure TfrmMain.FormDestroy(Sender: TObject);
begin
  { vstNav.Items.Clear will trigger vstNavDeletion for each node,
    freeing all PNodeData allocations before engine shutdown. }
  vstNav.Items.Clear;
  ShutdownEngine;
end;

procedure TfrmMain.FormCloseQuery(Sender: TObject; var CanClose: Boolean);
begin
  { Could prompt for unsaved changes here in the future }
  CanClose := True;
end;

{ --------------------------------------------------------------------------- }
{ T19: Node data allocation and lazy-loading                                    }
{ --------------------------------------------------------------------------- }

function TfrmMain.AllocNodeData(AType: TNodeType; APlugin, AGroup,
  ARecord: Integer): PNodeData;
begin
  New(Result);
  Result^.NodeType := AType;
  Result^.PluginIndex := APlugin;
  Result^.GroupIndex := AGroup;
  Result^.RecordIndex := ARecord;
  Result^.Loaded := False;
  Result^.RecordCount := 0;
end;

procedure TfrmMain.vstNavDeletion(Sender: TObject; Node: TTreeNode);
var
  ND: PNodeData;
begin
  if Node.Data <> nil then
  begin
    ND := PNodeData(Node.Data);
    Dispose(ND);
    Node.Data := nil;
  end;
end;

procedure TfrmMain.vstNavExpanding(Sender: TObject; Node: TTreeNode;
  var AllowExpansion: Boolean);
var
  ND: PNodeData;
begin
  AllowExpansion := True;

  if Node.Data = nil then
    Exit;

  ND := PNodeData(Node.Data);

  { Only lazy-load for group nodes that have not been loaded yet }
  if (ND^.NodeType = ntGroup) and (not ND^.Loaded) then
  begin
    LoadGroupRecords(Node, ND);
  end;
end;

procedure TfrmMain.LoadGroupRecords(ANode: TTreeNode; AData: PNodeData);
var
  RecCount: Integer;
  I: Integer;
  SigBuf: array[0..31] of Char;
  EdidBuf: array[0..255] of Char;
  SigLen, EdidLen: Integer;
  Sig, EditorID, NodeText: string;
  FormID: Cardinal;
  RecNode: TTreeNode;
  RecND: PNodeData;
begin
  if AData^.Loaded then
    Exit;

  RecCount := AData^.RecordCount;
  if RecCount <= 0 then
  begin
    { Re-query in case it was not cached }
    RecCount := xedit_group_record_count(AData^.PluginIndex, AData^.GroupIndex);
    if RecCount <= 0 then
    begin
      AData^.Loaded := True;
      Exit;
    end;
  end;

  SetStatusText('Loading ' + IntToStr(RecCount) + ' records...');
  Application.ProcessMessages;

  vstNav.Items.BeginUpdate;
  try
    { Remove the placeholder child if present }
    if (ANode.Count = 1) and (ANode.Items[0].Data = nil) and
       (ANode.Items[0].Text = '...') then
      ANode.Items[0].Delete;

    for I := 0 to RecCount - 1 do
    begin
      { Get record signature }
      FillChar(SigBuf, SizeOf(SigBuf), 0);
      SigLen := xedit_record_signature(AData^.PluginIndex, AData^.GroupIndex,
        I, @SigBuf[0], SizeOf(SigBuf));
      if SigLen > 0 then
        Sig := StrPas(@SigBuf[0])
      else
        Sig := '????';

      { Get FormID }
      FormID := xedit_record_form_id(AData^.PluginIndex, AData^.GroupIndex, I);

      { Get EditorID }
      FillChar(EdidBuf, SizeOf(EdidBuf), 0);
      EdidLen := xedit_record_editor_id(AData^.PluginIndex, AData^.GroupIndex,
        I, @EdidBuf[0], SizeOf(EdidBuf));
      if EdidLen > 0 then
        EditorID := StrPas(@EdidBuf[0])
      else
        EditorID := '';

      { Build display text: "SIGN [FormID] EditorID" }
      NodeText := Sig + ' [' + IntToHex(FormID, 8) + ']';
      if EditorID <> '' then
        NodeText := NodeText + ' ' + EditorID;

      RecNode := vstNav.Items.AddChild(ANode, NodeText);
      RecNode.ImageIndex := 2;

      { Attach node data }
      RecND := AllocNodeData(ntRecord, AData^.PluginIndex,
        AData^.GroupIndex, I);
      RecND^.Loaded := True;
      RecNode.Data := RecND;

      { Keep UI responsive for large groups }
      if (I > 0) and ((I mod 500) = 0) then
      begin
        SetStatusText('Loading records... ' + IntToStr(I) + '/' + IntToStr(RecCount));
        Application.ProcessMessages;
      end;
    end;

    AData^.Loaded := True;
  finally
    vstNav.Items.EndUpdate;
  end;

  SetStatusText('Ready - ' + IntToStr(RecCount) + ' records loaded');
end;

{ --------------------------------------------------------------------------- }
{ T19: Navigation selection handler                                             }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.vstNavChange(Sender: TObject; Node: TTreeNode);
var
  ND: PNodeData;
begin
  if Node = nil then
  begin
    LogMessage('Selection: nil node');
    ClearRecordView;
    Exit;
  end;

  LogMessage('Selection: "' + Node.Text + '" Data=' + BoolToStr(Node.Data <> nil, 'yes', 'nil'));

  if Node.Data = nil then
    Exit;

  ND := PNodeData(Node.Data);
  LogMessage('  NodeType=' + IntToStr(Ord(ND^.NodeType)) +
    ' P=' + IntToStr(ND^.PluginIndex) +
    ' G=' + IntToStr(ND^.GroupIndex) +
    ' R=' + IntToStr(ND^.RecordIndex));

  case ND^.NodeType of
    ntPlugin:
      begin
        SetStatusText('Plugin [' + IntToStr(ND^.PluginIndex) + '] selected');
        ClearRecordView;
      end;
    ntGroup:
      begin
        SetStatusText('Group [' + IntToStr(ND^.GroupIndex) + '] in plugin [' +
          IntToStr(ND^.PluginIndex) + ']');
        ClearRecordView;
      end;
    ntRecord:
      begin
        LogMessage('  Record selected - populating view...');
        { Avoid redundant reload if same record is already displayed }
        if (ND^.PluginIndex = FViewPluginIdx) and
           (ND^.GroupIndex = FViewGroupIdx) and
           (ND^.RecordIndex = FViewRecordIdx) then
        begin
          LogMessage('  Same record, skipping reload');
          Exit;
        end;

        FViewPluginIdx := ND^.PluginIndex;
        FViewGroupIdx := ND^.GroupIndex;
        FViewRecordIdx := ND^.RecordIndex;

        PopulateRecordView(ND^.PluginIndex, ND^.GroupIndex, ND^.RecordIndex);
        PopulateReferencedBy(ND^.PluginIndex, ND^.GroupIndex, ND^.RecordIndex);
        pgMain.ActivePage := tbsView;
        LogMessage('  View populated, switched to View tab');
      end;
  end;
end;

{ --------------------------------------------------------------------------- }
{ Drag-and-drop support for plugin load order reordering                        }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.vstNavDragOver(Sender, Source: TObject; X, Y: Integer;
  State: TDragState; var Accept: Boolean);
var
  TargetNode: TTreeNode;
  SourceND, TargetND: PNodeData;
begin
  Accept := False;

  { Only accept drags originating from the nav tree itself }
  if Source <> vstNav then
    Exit;

  { Must have a valid selected (source) node }
  if vstNav.Selected = nil then
    Exit;
  if vstNav.Selected.Data = nil then
    Exit;

  { Find the target node under the cursor }
  TargetNode := vstNav.GetNodeAt(X, Y);
  if TargetNode = nil then
    Exit;
  if TargetNode.Data = nil then
    Exit;

  { Do not allow dropping on self }
  if TargetNode = vstNav.Selected then
    Exit;

  SourceND := PNodeData(vstNav.Selected.Data);
  TargetND := PNodeData(TargetNode.Data);

  { Accept only if both source and target are plugin nodes }
  Accept := (SourceND^.NodeType = ntPlugin) and (TargetND^.NodeType = ntPlugin);
end;

procedure TfrmMain.vstNavDragDrop(Sender, Source: TObject; X, Y: Integer);
var
  TargetNode: TTreeNode;
  SourceNode: TTreeNode;
  SourceND, TargetND: PNodeData;
begin
  if Source <> vstNav then
    Exit;

  SourceNode := vstNav.Selected;
  if SourceNode = nil then
    Exit;
  if SourceNode.Data = nil then
    Exit;

  TargetNode := vstNav.GetNodeAt(X, Y);
  if TargetNode = nil then
    Exit;
  if TargetNode.Data = nil then
    Exit;
  if TargetNode = SourceNode then
    Exit;

  SourceND := PNodeData(SourceNode.Data);
  TargetND := PNodeData(TargetNode.Data);

  { Only reorder plugin nodes }
  if (SourceND^.NodeType <> ntPlugin) or (TargetND^.NodeType <> ntPlugin) then
    Exit;

  LogMessage('Reordering plugin [' + IntToStr(SourceND^.PluginIndex) +
    '] "' + SourceNode.Text + '" before plugin [' +
    IntToStr(TargetND^.PluginIndex) + '] "' + TargetNode.Text + '"');

  { Move the source node before the target node (visual reorder) }
  SourceNode.MoveTo(TargetNode, naInsert);

  LogMessage('Plugin load order reordered (visual only - engine reorder not yet implemented).');
  SetStatusText('Load order changed (visual)');
end;

{ --------------------------------------------------------------------------- }
{ Async Referenced By build timer                                               }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.tmrRefByBuildTimer(Sender: TObject);
var
  Status: Integer;
begin
  if not Assigned(xedit_refby_build_status) then
  begin
    tmrRefByBuild.Enabled := False;
    Exit;
  end;

  Status := xedit_refby_build_status();
  case Status of
    2: begin  { Done }
      tmrRefByBuild.Enabled := False;
      LogMessage('Referenced By index built successfully (async).');
      SetStatusText('Referenced By index ready.');
    end;
    -1: begin  { Error }
      tmrRefByBuild.Enabled := False;
      LogMessage('WARNING: Referenced By index build failed (async).');
      SetStatusText('Referenced By index build failed.');
    end;
    { 1 = still building, keep polling }
  end;
end;

{ --------------------------------------------------------------------------- }
{ T19: Search integration                                                       }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.edFormIDSearchKeyPress(Sender: TObject; var Key: Char);
begin
  if Key = #13 then
  begin
    Key := #0;  { Suppress the beep }
    DoFormIDSearch;
  end;
end;

procedure TfrmMain.edEditorIDSearchKeyPress(Sender: TObject; var Key: Char);
begin
  if Key = #13 then
  begin
    Key := #0;  { Suppress the beep }
    DoEditorIDSearch;
  end;
end;

procedure TfrmMain.DoFormIDSearch;
var
  SearchText: string;
  FormID: Cardinal;
  PluginIdx: Integer;
  PluginName: string;
  GroupIdx, RecordIdx: Integer;
  GroupNode: TTreeNode;
  RecNode: TTreeNode;
  Ret: Integer;
begin
  SearchText := Trim(edFormIDSearch.Text);
  if SearchText = '' then
    Exit;

  { Strip optional leading 0x or $ prefix }
  if (Length(SearchText) > 2) and
     ((Copy(SearchText, 1, 2) = '0x') or (Copy(SearchText, 1, 2) = '0X')) then
    SearchText := Copy(SearchText, 3, Length(SearchText));
  if (Length(SearchText) > 0) and (SearchText[1] = '$') then
    SearchText := Copy(SearchText, 2, Length(SearchText));

  { Parse hex FormID }
  try
    FormID := Cardinal(StrToInt64('$' + SearchText));
  except
    on E: Exception do
    begin
      LogMessage('FormID search: invalid hex value "' + edFormIDSearch.Text + '"');
      SetStatusText('Invalid FormID format');
      Exit;
    end;
  end;

  if not FEngineReady then
  begin
    LogMessage('FormID search: engine not ready');
    SetStatusText('Engine not ready');
    Exit;
  end;

  if not Assigned(xedit_search_form_id) then
  begin
    LogMessage('FormID search: xedit_search_form_id not available');
    SetStatusText('Search function not available');
    Exit;
  end;

  { Determine which plugin to search in (use selected node or first loaded) }
  PluginIdx := GetSelectedPluginIndex(PluginName);
  if PluginIdx < 0 then
    PluginIdx := 0;  { Default to first plugin }

  GroupIdx := -1;
  RecordIdx := -1;

  Ret := xedit_search_form_id(PluginIdx, FormID, @GroupIdx, @RecordIdx);
  if (Ret < 0) or (GroupIdx < 0) or (RecordIdx < 0) then
  begin
    LogMessage('FormID search: [' + IntToHex(FormID, 8) + '] not found in plugin ' +
      IntToStr(PluginIdx));
    SetStatusText('FormID [' + IntToHex(FormID, 8) + '] not found');
    Exit;
  end;

  LogMessage('FormID search: [' + IntToHex(FormID, 8) + '] found at group=' +
    IntToStr(GroupIdx) + ' record=' + IntToStr(RecordIdx));

  { Ensure the group node is expanded (triggers lazy load if needed) }
  EnsureGroupExpanded(PluginIdx, GroupIdx, GroupNode);
  if GroupNode = nil then
  begin
    LogMessage('FormID search: could not find group node');
    SetStatusText('Search error: group node not found');
    Exit;
  end;

  { Find the record node within the expanded group }
  RecNode := FindRecordNode(GroupNode, RecordIdx);
  if RecNode <> nil then
    SelectAndFocusNode(RecNode)
  else
  begin
    LogMessage('FormID search: record node not found after expansion');
    SetStatusText('Search error: record node not found');
  end;
end;

procedure TfrmMain.DoEditorIDSearch;
var
  Query: string;
  PluginIdx: Integer;
  PluginName: string;
  ResultsBuf: array[0..MAX_EDITOR_ID_RESULTS * 2 - 1] of Integer;
  ResultCount: Integer;
  GroupIdx, RecordIdx: Integer;
  GroupNode: TTreeNode;
  RecNode: TTreeNode;
begin
  Query := Trim(edEditorIDSearch.Text);
  if Query = '' then
    Exit;

  if not FEngineReady then
  begin
    LogMessage('EditorID search: engine not ready');
    SetStatusText('Engine not ready');
    Exit;
  end;

  if not Assigned(xedit_search_editor_id) then
  begin
    LogMessage('EditorID search: xedit_search_editor_id not available');
    SetStatusText('Search function not available');
    Exit;
  end;

  { Determine which plugin to search in }
  PluginIdx := GetSelectedPluginIndex(PluginName);
  if PluginIdx < 0 then
    PluginIdx := 0;

  FillChar(ResultsBuf, SizeOf(ResultsBuf), 0);
  ResultCount := xedit_search_editor_id(PluginIdx, PChar(Query),
    @ResultsBuf[0], MAX_EDITOR_ID_RESULTS);

  if ResultCount <= 0 then
  begin
    LogMessage('EditorID search: "' + Query + '" not found in plugin ' +
      IntToStr(PluginIdx));
    SetStatusText('EditorID "' + Query + '" not found');
    Exit;
  end;

  LogMessage('EditorID search: "' + Query + '" found ' + IntToStr(ResultCount) +
    ' result(s)');

  { Navigate to the first result.
    Results buffer layout: pairs of (group_idx, record_idx). }
  GroupIdx := ResultsBuf[0];
  RecordIdx := ResultsBuf[1];

  if ResultCount > 1 then
    SetStatusText('Found ' + IntToStr(ResultCount) + ' matches - showing first');

  { Ensure the group node is expanded }
  EnsureGroupExpanded(PluginIdx, GroupIdx, GroupNode);
  if GroupNode = nil then
  begin
    LogMessage('EditorID search: could not find group node');
    SetStatusText('Search error: group node not found');
    Exit;
  end;

  { Find and select the record node }
  RecNode := FindRecordNode(GroupNode, RecordIdx);
  if RecNode <> nil then
    SelectAndFocusNode(RecNode)
  else
  begin
    LogMessage('EditorID search: record node not found after expansion');
    SetStatusText('Search error: record node not found');
  end;
end;

{ Find the group node for a given plugin_idx + group_idx in the tree }
function TfrmMain.FindGroupNode(APluginIdx, AGroupIdx: Integer): TTreeNode;
var
  PluginNode, GroupNode: TTreeNode;
  ND: PNodeData;
begin
  Result := nil;

  { Iterate root (plugin) nodes }
  PluginNode := vstNav.Items.GetFirstNode;
  while PluginNode <> nil do
  begin
    if PluginNode.Data <> nil then
    begin
      ND := PNodeData(PluginNode.Data);
      if (ND^.NodeType = ntPlugin) and (ND^.PluginIndex = APluginIdx) then
      begin
        { Found the plugin; find the group child }
        GroupNode := PluginNode.GetFirstChild;
        while GroupNode <> nil do
        begin
          if GroupNode.Data <> nil then
          begin
            ND := PNodeData(GroupNode.Data);
            if (ND^.NodeType = ntGroup) and (ND^.GroupIndex = AGroupIdx) then
            begin
              Result := GroupNode;
              Exit;
            end;
          end;
          GroupNode := GroupNode.GetNextSibling;
        end;
        Exit;  { Plugin found but group not found }
      end;
    end;
    PluginNode := PluginNode.GetNextSibling;
  end;
end;

{ Find a record node within an already-expanded group node by record index }
function TfrmMain.FindRecordNode(AGroupNode: TTreeNode;
  ARecordIdx: Integer): TTreeNode;
var
  Child: TTreeNode;
  ND: PNodeData;
begin
  Result := nil;
  Child := AGroupNode.GetFirstChild;
  while Child <> nil do
  begin
    if Child.Data <> nil then
    begin
      ND := PNodeData(Child.Data);
      if (ND^.NodeType = ntRecord) and (ND^.RecordIndex = ARecordIdx) then
      begin
        Result := Child;
        Exit;
      end;
    end;
    Child := Child.GetNextSibling;
  end;
end;

{ Ensure a group node is expanded (triggering lazy load if needed) }
procedure TfrmMain.EnsureGroupExpanded(APluginIdx, AGroupIdx: Integer;
  out AGroupNode: TTreeNode);
var
  ND: PNodeData;
begin
  AGroupNode := FindGroupNode(APluginIdx, AGroupIdx);
  if AGroupNode = nil then
    Exit;

  { Ensure plugin parent is expanded }
  if (AGroupNode.Parent <> nil) and (not AGroupNode.Parent.Expanded) then
    AGroupNode.Parent.Expand(False);

  { Expanding the group will trigger vstNavExpanding -> LoadGroupRecords }
  if not AGroupNode.Expanded then
    AGroupNode.Expand(False);

  { Double-check that data was loaded }
  if AGroupNode.Data <> nil then
  begin
    ND := PNodeData(AGroupNode.Data);
    if not ND^.Loaded then
      LoadGroupRecords(AGroupNode, ND);
  end;
end;

procedure TfrmMain.SelectAndFocusNode(ANode: TTreeNode);
begin
  vstNav.Selected := ANode;
  ANode.MakeVisible;
  vstNav.SetFocus;
end;

{ --------------------------------------------------------------------------- }
{ Menu handlers                                                                 }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.mniFileOpenClick(Sender: TObject);
var
  GameExts: string;
begin
  GameExts := GetGameFilterExtensions;
  dlgOpenPlugin.Filter := 'Plugin files (' + GameExts + ')|' + GameExts +
    '|All files (*.*)|*.*';
  dlgOpenPlugin.Title := 'Open Plugin - ' + GetSelectedGameName;

  if dlgOpenPlugin.Execute then
  begin
    LogMessage('Opening file: ' + dlgOpenPlugin.FileName);
    LoadPluginFile(dlgOpenPlugin.FileName);
  end;
end;

procedure TfrmMain.mniFileMO2OpenClick(Sender: TObject);
begin
  dlgSelectMO2Dir.Title := 'Select MO2 Installation Folder';
  if dlgSelectMO2Dir.Execute then
  begin
    LogMessage('Selected MO2 folder: ' + dlgSelectMO2Dir.FileName);
    LoadMO2Folder(dlgSelectMO2Dir.FileName);
  end;
end;

procedure TfrmMain.mniFileExitClick(Sender: TObject);
begin
  Close;
end;

procedure TfrmMain.mniHelpAboutClick(Sender: TObject);
begin
  MessageDlg('About xEdit',
    'xEdit (Lazarus/LCL Port)' + LineEnding +
    'Version ' + APP_VERSION + LineEnding +
    LineEnding +
    'Advanced graphical plugin editor and conflict detector' + LineEnding +
    'for Bethesda game engine files.' + LineEnding +
    LineEnding +
    'Rust core engine with cross-platform GUI.' + LineEnding +
    LineEnding +
    'Original xEdit by ElminsterAU' + LineEnding +
    'Lazarus port and Rust core by the xEdit team' + LineEnding +
    LineEnding +
    'Discord: https://discord.gg/5t8RnNQ',
    mtInformation, [mbOK], 0);
end;

procedure TfrmMain.cobGameChange(Sender: TObject);
begin
  { Shut down any existing engine session when game changes }
  if FEngineReady then
  begin
    ShutdownEngine;
    vstNav.Items.Clear;
    LogMessage('Game changed to ' + GetSelectedGameName + '. Previous session closed.');
  end;
  SetStatusText('Game: ' + GetSelectedGameName + ' - Ready');
  LogMessage('Selected game: ' + GetSelectedGameName);
end;

{ --------------------------------------------------------------------------- }
{ Engine interaction                                                            }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.InitEngine;
var
  GameName: string;
  DataPath: string;
  Ret: Integer;
begin
  if FEngineReady then
    Exit;

  { Try to load the shared library if not already loaded }
  if not xedit_ffi_loaded then
  begin
    LogMessage('Loading xedit_core library...');
    SetStatusText('Loading xedit_core library...');
    Application.ProcessMessages;

    if not xedit_ffi_load('') then
    begin
      LogMessage('ERROR: Could not load ' + XEDIT_CORE_LIB);
      SetStatusText('Error: xedit_core library not found');
      MessageDlg('Library Not Found',
        'Could not load the xedit_core shared library (' + XEDIT_CORE_LIB + ').' + LineEnding +
        LineEnding +
        'The Rust core library has not been built yet.' + LineEnding +
        'Build it with: cargo build --release' + LineEnding +
        'Then copy the library to the application directory.',
        mtError, [mbOK], 0);
      Exit;
    end;
    LogMessage('Library loaded successfully.');
  end;

  GameName := GetSelectedGameName;
  { For now, use empty data path - the engine will use default game paths }
  DataPath := '';

  SetStatusText('Initializing ' + GameName + ' engine...');
  LogMessage('Initializing engine for ' + GameName + '...');
  Application.ProcessMessages;

  try
    if not Assigned(xedit_init) then
    begin
      LogMessage('ERROR: xedit_init function not found in library');
      SetStatusText('Error: Incompatible library version');
      Exit;
    end;

    Ret := xedit_init(PChar(GameName), PChar(DataPath), nil);
    if Ret < 0 then
    begin
      LogMessage('ERROR: xedit_init failed with code ' + IntToStr(Ret));
      SetStatusText('Error: Engine initialization failed (code ' + IntToStr(Ret) + ')');
      MessageDlg('Engine Error',
        'Failed to initialize the xEdit engine for ' + GameName + '.' + LineEnding +
        'Error code: ' + IntToStr(Ret) + LineEnding + LineEnding +
        'Make sure the xedit_core library is available.',
        mtError, [mbOK], 0);
      Exit;
    end;
    FEngineReady := True;
    FEngineHandle := Pointer(PtrInt(Ret));
    FCurrentGame := GameName;
    LogMessage('Engine initialized successfully for ' + GameName + '.');
    SetStatusText('Engine ready - ' + GameName);
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION during init: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Engine Error',
        'Exception during engine initialization:' + LineEnding + E.Message + LineEnding +
        LineEnding + 'The xedit_core shared library may not be built yet.',
        mtError, [mbOK], 0);
    end;
  end;
end;

procedure TfrmMain.ShutdownEngine;
begin
  if not FEngineReady then
    Exit;

  try
    if Assigned(xedit_shutdown) then
      xedit_shutdown();
    LogMessage('Engine shut down.');
  except
    on E: Exception do
      LogMessage('Warning: Exception during shutdown: ' + E.Message);
  end;

  FEngineReady := False;
  FEngineHandle := nil;
  FCurrentGame := '';
end;

procedure TfrmMain.LoadPluginFile(const AFilePath: string);
var
  PluginIndex: Integer;
  FileName: string;
begin
  FileName := ExtractFileName(AFilePath);

  { Ensure engine is initialized }
  if not FEngineReady then
    InitEngine;

  if not FEngineReady then
  begin
    LogMessage('Cannot load plugin - engine not ready.');
    Exit;
  end;

  SetStatusText('Loading ' + FileName + '...');
  LogMessage('Loading plugin: ' + AFilePath);
  Application.ProcessMessages;

  try
    if not Assigned(xedit_load_plugin) then
    begin
      LogMessage('ERROR: xedit_load_plugin not available');
      SetStatusText('Error: Incompatible library');
      Exit;
    end;
    PluginIndex := xedit_load_plugin(PChar(AFilePath));
    if PluginIndex < 0 then
    begin
      LogMessage('ERROR: xedit_load_plugin failed with code ' + IntToStr(PluginIndex));
      SetStatusText('Error loading ' + FileName);
      MessageDlg('Load Error',
        'Failed to load plugin: ' + FileName + LineEnding +
        'Error code: ' + IntToStr(PluginIndex),
        mtError, [mbOK], 0);
      Exit;
    end;

    LogMessage('Plugin loaded: ' + FileName + ' (index ' + IntToStr(PluginIndex) + ')');
    PopulateNavTree(PluginIndex, FileName);
    SetStatusText('Ready - ' + FileName + ' loaded');
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION loading plugin: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Load Error',
        'Exception while loading plugin:' + LineEnding + E.Message,
        mtError, [mbOK], 0);
    end;
  end;
end;

{ T19: Populate the nav tree with lazy-loading architecture.
  Only creates plugin and group nodes. Group nodes get a placeholder child
  so the expand arrow is shown. Records are loaded on demand via
  vstNavExpanding -> LoadGroupRecords. }
procedure TfrmMain.PopulateNavTree(APluginIndex: Integer; const AFileName: string);
var
  GroupCount: Integer;
  I: Integer;
  SigBuf: array[0..31] of Char;
  NameBuf: array[0..255] of Char;
  SigLen, NameLen: Integer;
  Sig, GrpName, NodeText: string;
  PluginNode, GroupNode, PlaceholderNode: TTreeNode;
  RecCount: Integer;
  PluginND, GroupND: PNodeData;
begin
  { Add plugin as a root node with associated data }
  PluginNode := vstNav.Items.AddChild(nil, '[' + IntToStr(APluginIndex) + '] ' + AFileName);
  PluginNode.ImageIndex := 0;
  PluginND := AllocNodeData(ntPlugin, APluginIndex, -1, -1);
  PluginND^.Loaded := True;
  PluginNode.Data := PluginND;

  { Query group count }
  GroupCount := xedit_plugin_group_count(APluginIndex);
  if GroupCount < 0 then
  begin
    vstNav.Items.AddChild(PluginNode, '(error reading groups)');
    Exit;
  end;

    for I := 0 to GroupCount - 1 do
    begin
      { Get group signature }
      FillChar(SigBuf, SizeOf(SigBuf), 0);
      SigLen := xedit_group_signature(APluginIndex, I, @SigBuf[0], SizeOf(SigBuf));
      if SigLen > 0 then
        Sig := StrPas(@SigBuf[0])
      else
        Sig := '????';

      { Get group name }
      FillChar(NameBuf, SizeOf(NameBuf), 0);
      NameLen := xedit_group_name(APluginIndex, I, @NameBuf[0], SizeOf(NameBuf));
      if NameLen > 0 then
        GrpName := StrPas(@NameBuf[0])
      else
        GrpName := '';

      { Get record count for virtual display (without loading records) }
      RecCount := xedit_group_record_count(APluginIndex, I);

      { Build display text matching xEdit style: "Friendly Name" }
      NodeText := SignatureToFriendlyName(Sig);

      GroupNode := vstNav.Items.AddChild(PluginNode, NodeText);
      GroupNode.ImageIndex := 1;

      { Attach group node data for lazy loading }
      GroupND := AllocNodeData(ntGroup, APluginIndex, I, -1);
      GroupND^.Loaded := False;
      GroupND^.RecordCount := RecCount;
      GroupNode.Data := GroupND;

      { Add a placeholder child so the expand arrow is visible.
        This placeholder will be removed when records are actually loaded. }
      if RecCount > 0 then
      begin
        PlaceholderNode := vstNav.Items.AddChild(GroupNode, '...');
        PlaceholderNode.Data := nil;  { No data = placeholder sentinel }
      end;
    end;
end;

{ --------------------------------------------------------------------------- }
{ Record view (T20 placeholder)                                                 }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.PopulateRecordView(APluginIdx, AGroupIdx, ARecordIdx: Integer);
var
  SubCount, I, SigLen, DataLen, DisplayLen: Integer;
  SigBuf: array[0..31] of Char;
  EdidBuf: array[0..511] of Char;
  RecSigBuf: array[0..31] of Char;
  DataBuf: array[0..4095] of Char;
  Sig, EditorID, RecSig, NodeText, DataStr: string;
  FormID: Cardinal;
  RootNode, SubNode: TTreeNode;
begin
  LogMessage('PopulateRecordView P=' + IntToStr(APluginIdx) +
    ' G=' + IntToStr(AGroupIdx) + ' R=' + IntToStr(ARecordIdx));

  vstView.Items.BeginUpdate;
  try
    vstView.Items.Clear;

    { Get record header info }
    RecSig := '????';
    if Assigned(xedit_record_signature) then
    begin
      FillChar(RecSigBuf, SizeOf(RecSigBuf), 0);
      SigLen := xedit_record_signature(APluginIdx, AGroupIdx, ARecordIdx,
        @RecSigBuf[0], SizeOf(RecSigBuf));
      if SigLen > 0 then
        RecSig := StrPas(@RecSigBuf[0]);
    end;

    FormID := 0;
    if Assigned(xedit_record_form_id) then
      FormID := xedit_record_form_id(APluginIdx, AGroupIdx, ARecordIdx);

    EditorID := '';
    if Assigned(xedit_record_editor_id) then
    begin
      FillChar(EdidBuf, SizeOf(EdidBuf), 0);
      SigLen := xedit_record_editor_id(APluginIdx, AGroupIdx, ARecordIdx,
        @EdidBuf[0], SizeOf(EdidBuf));
      if SigLen > 0 then
        EditorID := StrPas(@EdidBuf[0]);
    end;

    { Root node: [FormID] SIGNATURE - EditorID }
    NodeText := '[' + IntToHex(FormID, 8) + '] ' + RecSig;
    if EditorID <> '' then
      NodeText := NodeText + ' - ' + EditorID;
    RootNode := vstView.Items.AddChild(nil, NodeText);

    { Enumerate subrecords }
    if not Assigned(xedit_record_subrecord_count) then
    begin
      vstView.Items.AddChild(RootNode, '[subrecord API not available]');
      RootNode.Expand(False);
      Exit;
    end;

    SubCount := xedit_record_subrecord_count(APluginIdx, AGroupIdx, ARecordIdx);
    if SubCount < 0 then
    begin
      vstView.Items.AddChild(RootNode, '[error reading subrecords: ' +
        IntToStr(SubCount) + ']');
      RootNode.Expand(False);
      Exit;
    end;

    for I := 0 to SubCount - 1 do
    begin
      { Get subrecord signature }
      Sig := '????';
      if Assigned(xedit_subrecord_signature) then
      begin
        FillChar(SigBuf, SizeOf(SigBuf), 0);
        SigLen := xedit_subrecord_signature(APluginIdx, AGroupIdx, ARecordIdx,
          I, @SigBuf[0], SizeOf(SigBuf));
        if SigLen > 0 then
          Sig := StrPas(@SigBuf[0]);
      end;

      { Get subrecord size }
      DataLen := 0;
      if Assigned(xedit_subrecord_size) then
        DataLen := xedit_subrecord_size(APluginIdx, AGroupIdx, ARecordIdx, I);

      { Get subrecord data }
      DataStr := '';
      if (DataLen > 0) and Assigned(xedit_subrecord_data) then
      begin
        DisplayLen := DataLen;
        if DisplayLen > SizeOf(DataBuf) then
          DisplayLen := SizeOf(DataBuf);
        FillChar(DataBuf, SizeOf(DataBuf), 0);
        xedit_subrecord_data(APluginIdx, AGroupIdx, ARecordIdx, I,
          @DataBuf[0], DisplayLen);

        if IsTextSubrecord(Sig) then
          DataStr := TryDecodeString(@DataBuf[0], DisplayLen)
        else
        begin
          if DisplayLen > 32 then
            DataStr := BytesToHex(@DataBuf[0], 32) + '...'
          else
            DataStr := BytesToHex(@DataBuf[0], DisplayLen);
        end;
      end;

      { Build subrecord node text }
      NodeText := Sig + ' - [' + IntToStr(DataLen) + ' bytes]';
      if DataStr <> '' then
        NodeText := NodeText + ' - ' + DataStr;

      SubNode := vstView.Items.AddChild(RootNode, NodeText);
    end;

    RootNode.Expand(False);
  finally
    vstView.Items.EndUpdate;
  end;

  UpdateRecordInfo(APluginIdx, AGroupIdx, ARecordIdx);
  SetStatusText('Record [' + IntToHex(FormID, 8) + '] ' + RecSig);
end;

procedure TfrmMain.UpdateRecordInfo(APluginIdx, AGroupIdx, ARecordIdx: Integer);
var
  SigBuf: array[0..31] of Char;
  EdidBuf: array[0..511] of Char;
  SigLen, SubCount: Integer;
  FormID: Cardinal;
  RecSig, EditorID: string;
begin
  mmoInfo.Lines.Clear;
  mmoInfo.Lines.Add('Record Details');
  mmoInfo.Lines.Add('==============');
  mmoInfo.Lines.Add('');

  RecSig := '????';
  if Assigned(xedit_record_signature) then
  begin
    FillChar(SigBuf, SizeOf(SigBuf), 0);
    SigLen := xedit_record_signature(APluginIdx, AGroupIdx, ARecordIdx,
      @SigBuf[0], SizeOf(SigBuf));
    if SigLen > 0 then
      RecSig := StrPas(@SigBuf[0]);
  end;

  FormID := 0;
  if Assigned(xedit_record_form_id) then
    FormID := xedit_record_form_id(APluginIdx, AGroupIdx, ARecordIdx);

  EditorID := '';
  if Assigned(xedit_record_editor_id) then
  begin
    FillChar(EdidBuf, SizeOf(EdidBuf), 0);
    SigLen := xedit_record_editor_id(APluginIdx, AGroupIdx, ARecordIdx,
      @EdidBuf[0], SizeOf(EdidBuf));
    if SigLen > 0 then
      EditorID := StrPas(@EdidBuf[0]);
  end;

  SubCount := 0;
  if Assigned(xedit_record_subrecord_count) then
    SubCount := xedit_record_subrecord_count(APluginIdx, AGroupIdx, ARecordIdx);

  mmoInfo.Lines.Add('Signature:     ' + RecSig);
  mmoInfo.Lines.Add('FormID:        ' + IntToHex(FormID, 8));
  if EditorID <> '' then
    mmoInfo.Lines.Add('Editor ID:     ' + EditorID);
  mmoInfo.Lines.Add('Plugin Index:  ' + IntToStr(APluginIdx));
  mmoInfo.Lines.Add('Group Index:   ' + IntToStr(AGroupIdx));
  mmoInfo.Lines.Add('Record Index:  ' + IntToStr(ARecordIdx));
  mmoInfo.Lines.Add('Subrecords:    ' + IntToStr(SubCount));
end;

procedure TfrmMain.ClearRecordView;
begin
  FViewPluginIdx := -1;
  FViewGroupIdx := -1;
  FViewRecordIdx := -1;
  vstView.Items.Clear;
  mmoInfo.Lines.Clear;
  lvReferencedBy.Items.Clear;
end;

procedure TfrmMain.PopulateReferencedBy(APluginIdx, AGroupIdx, ARecordIdx: Integer);
var
  RefCount, I, Ret: Integer;
  RefPluginIdx, RefGroupIdx, RefRecordIdx: Int32;
  SigBuf: array[0..31] of Char;
  EdidBuf: array[0..255] of Char;
  NameBuf: array[0..511] of Char;
  SigLen, EdidLen, NameLen: Integer;
  Sig, EditorID, PluginName, RecordText: string;
  FormID: Cardinal;
  ListItem: TListItem;
begin
  lvReferencedBy.Items.Clear;

  { Check that the FFI functions are available }
  if not Assigned(xedit_record_refby_count) then
  begin
    LogMessage('PopulateReferencedBy: xedit_record_refby_count not available');
    Exit;
  end;

  if not Assigned(xedit_record_refby_entry) then
  begin
    LogMessage('PopulateReferencedBy: xedit_record_refby_entry not available');
    Exit;
  end;

  RefCount := xedit_record_refby_count(APluginIdx, AGroupIdx, ARecordIdx);
  if RefCount <= 0 then
  begin
    if RefCount < 0 then
      LogMessage('PopulateReferencedBy: xedit_record_refby_count returned error ' +
        IntToStr(RefCount));
    Exit;
  end;

  LogMessage('PopulateReferencedBy: ' + IntToStr(RefCount) + ' reference(s) found');

  lvReferencedBy.Items.BeginUpdate;
  try
    for I := 0 to RefCount - 1 do
    begin
      RefPluginIdx := -1;
      RefGroupIdx := -1;
      RefRecordIdx := -1;

      Ret := xedit_record_refby_entry(APluginIdx, AGroupIdx, ARecordIdx, I,
        @RefPluginIdx, @RefGroupIdx, @RefRecordIdx);
      if Ret <> 0 then
      begin
        LogMessage('PopulateReferencedBy: entry ' + IntToStr(I) + ' failed with code ' +
          IntToStr(Ret));
        Continue;
      end;

      { Get the referencing record's signature }
      Sig := '????';
      if Assigned(xedit_record_signature) then
      begin
        FillChar(SigBuf, SizeOf(SigBuf), 0);
        SigLen := xedit_record_signature(RefPluginIdx, RefGroupIdx, RefRecordIdx,
          @SigBuf[0], SizeOf(SigBuf));
        if SigLen > 0 then
          Sig := StrPas(@SigBuf[0]);
      end;

      { Get FormID }
      FormID := 0;
      if Assigned(xedit_record_form_id) then
        FormID := xedit_record_form_id(RefPluginIdx, RefGroupIdx, RefRecordIdx);

      { Get EditorID }
      EditorID := '';
      if Assigned(xedit_record_editor_id) then
      begin
        FillChar(EdidBuf, SizeOf(EdidBuf), 0);
        EdidLen := xedit_record_editor_id(RefPluginIdx, RefGroupIdx, RefRecordIdx,
          @EdidBuf[0], SizeOf(EdidBuf));
        if EdidLen > 0 then
          EditorID := StrPas(@EdidBuf[0]);
      end;

      { Get plugin filename }
      PluginName := 'Plugin ' + IntToStr(RefPluginIdx);
      if Assigned(xedit_plugin_filename) then
      begin
        FillChar(NameBuf, SizeOf(NameBuf), 0);
        NameLen := xedit_plugin_filename(RefPluginIdx, @NameBuf[0], SizeOf(NameBuf));
        if NameLen > 0 then
          PluginName := StrPas(@NameBuf[0]);
      end;

      { Build the Record column text: "[FormID] EditorID" }
      RecordText := '[' + IntToHex(FormID, 8) + ']';
      if EditorID <> '' then
        RecordText := RecordText + ' ' + EditorID;

      { Add list item with columns: Record, Signature, File }
      ListItem := lvReferencedBy.Items.Add;
      ListItem.Caption := RecordText;
      ListItem.SubItems.Add(Sig);
      ListItem.SubItems.Add(PluginName);
    end;
  finally
    lvReferencedBy.Items.EndUpdate;
  end;
end;

function TfrmMain.BytesToHex(ABuf: PChar; ALen: Integer): string;
var
  I: Integer;
begin
  Result := '';
  for I := 0 to ALen - 1 do
  begin
    if I > 0 then
      Result := Result + ' ';
    Result := Result + IntToHex(Ord(ABuf[I]), 2);
  end;
end;

function TfrmMain.TryDecodeString(ABuf: PChar; ALen: Integer): string;
var
  I: Integer;
  ActualLen: Integer;
begin
  { Find null terminator }
  ActualLen := ALen;
  for I := 0 to ALen - 1 do
  begin
    if ABuf[I] = #0 then
    begin
      ActualLen := I;
      Break;
    end;
  end;

  if ActualLen = 0 then
    Result := '""'
  else
  begin
    SetString(Result, ABuf, ActualLen);
    Result := '"' + Result + '"';
  end;
end;

function TfrmMain.IsTextSubrecord(const ASig: string): Boolean;
begin
  Result := (ASig = 'EDID') or (ASig = 'FULL') or (ASig = 'DESC') or
            (ASig = 'MODL') or (ASig = 'ICON') or (ASig = 'MICO') or
            (ASig = 'MOD2') or (ASig = 'MOD3') or (ASig = 'MOD4') or
            (ASig = 'MOD5') or (ASig = 'NNAM') or (ASig = 'ANAM') or
            (ASig = 'INAM') or (ASig = 'ONAM') or (ASig = 'CNAM');
end;

{ --------------------------------------------------------------------------- }
{ Helpers                                                                       }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.SetStatusText(const AText: string);
begin
  if stbMain.Panels.Count > 0 then
    stbMain.Panels[0].Text := AText;
end;

procedure TfrmMain.LogMessage(const AMsg: string);
begin
  mmoMessages.Lines.Add('[' + FormatDateTime('hh:nn:ss', Now) + '] ' + AMsg);
end;

function TfrmMain.GetSelectedGameName: string;
begin
  if (cobGame.ItemIndex >= 0) and (cobGame.ItemIndex < cobGame.Items.Count) then
    Result := cobGame.Items[cobGame.ItemIndex]
  else
    Result := 'SSE';
end;

function TfrmMain.GetGameFilterExtensions: string;
var
  Game: string;
begin
  Game := GetSelectedGameName;
  if (Game = 'TES3') or (Game = 'Morrowind') then
    Result := '*.esp;*.esm'
  else if (Game = 'TES4') or (Game = 'Oblivion') then
    Result := '*.esp;*.esm'
  else
    Result := '*.esp;*.esm;*.esl';
end;

function TfrmMain.GetSelectedPluginIndex(out PluginName: string): Integer;
var
  Node: TTreeNode;
  ND: PNodeData;
  NodeText: string;
  BracketStart, BracketEnd: Integer;
begin
  Result := -1;
  PluginName := '';

  Node := vstNav.Selected;
  if Node = nil then
    Exit;

  { Walk up to the root node (plugin level) }
  while Node.Parent <> nil do
    Node := Node.Parent;

  { Try to get plugin index from node data first }
  if Node.Data <> nil then
  begin
    ND := PNodeData(Node.Data);
    if ND^.NodeType = ntPlugin then
    begin
      Result := ND^.PluginIndex;
      { Extract filename from display text }
      NodeText := Node.Text;
      BracketEnd := Pos(']', NodeText);
      if BracketEnd > 0 then
        PluginName := Trim(Copy(NodeText, BracketEnd + 1, Length(NodeText)))
      else
        PluginName := NodeText;
      Exit;
    end;
  end;

  { Fallback: parse "[index] filename" format from text }
  NodeText := Node.Text;
  BracketStart := Pos('[', NodeText);
  BracketEnd := Pos(']', NodeText);
  if (BracketStart > 0) and (BracketEnd > BracketStart) then
  begin
    Result := StrToIntDef(Copy(NodeText, BracketStart + 1,
      BracketEnd - BracketStart - 1), -1);
    PluginName := Trim(Copy(NodeText, BracketEnd + 1, Length(NodeText)));
  end;
end;

{ --------------------------------------------------------------------------- }
{ Plugin Cleaning Operations                                                    }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.DoQuickClean(Sender: TObject);
var
  PluginIndex: Integer;
  PluginName: string;
  ITMCount, UDRCount: Integer;
begin
  PluginIndex := GetSelectedPluginIndex(PluginName);
  if PluginIndex < 0 then
  begin
    MessageDlg('No Plugin Selected',
      'Please select a plugin node in the navigation tree first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if not FEngineReady then
  begin
    MessageDlg('Engine Not Ready',
      'The engine is not initialized. Load a plugin first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  { Confirm before cleaning }
  if MessageDlg('Quick Clean',
    'Clean ' + PluginName + '?' + LineEnding +
    LineEnding +
    'This will remove Identical-to-Master (ITM) records and' + LineEnding +
    'undelete/disable deleted references (UDR).' + LineEnding +
    LineEnding +
    'This will modify the plugin.',
    mtConfirmation, [mbYes, mbNo], 0) <> mrYes then
    Exit;

  SetStatusText('Cleaning ' + PluginName + '...');
  LogMessage('Starting Quick Clean for ' + PluginName + '...');
  Application.ProcessMessages;

  ITMCount := 0;
  UDRCount := 0;

  try
    { Remove ITM records }
    if Assigned(xedit_clean_itm) then
    begin
      LogMessage('  Removing ITM records...');
      Application.ProcessMessages;
      ITMCount := xedit_clean_itm(FEngineHandle, PluginIndex);
      if ITMCount < 0 then
      begin
        LogMessage('  ERROR: xedit_clean_itm returned ' + IntToStr(ITMCount));
        ITMCount := 0;
      end
      else
        LogMessage('  ITM records removed: ' + IntToStr(ITMCount));
    end
    else
      LogMessage('  WARNING: xedit_clean_itm not available in library');

    Application.ProcessMessages;

    { Undelete and disable references }
    if Assigned(xedit_clean_deleted) then
    begin
      LogMessage('  Undeleting and disabling references...');
      Application.ProcessMessages;
      UDRCount := xedit_clean_deleted(FEngineHandle, PluginIndex);
      if UDRCount < 0 then
      begin
        LogMessage('  ERROR: xedit_clean_deleted returned ' + IntToStr(UDRCount));
        UDRCount := 0;
      end
      else
        LogMessage('  Deleted references fixed: ' + IntToStr(UDRCount));
    end
    else
      LogMessage('  WARNING: xedit_clean_deleted not available in library');

    LogMessage('Quick Clean complete for ' + PluginName + '.');
    SetStatusText('Ready - Clean complete');

    { Show summary dialog }
    MessageDlg('Cleaning Complete',
      'Cleaning complete for ' + PluginName + LineEnding +
      LineEnding +
      'ITM records removed: ' + IntToStr(ITMCount) + LineEnding +
      'Deleted references fixed: ' + IntToStr(UDRCount),
      mtInformation, [mbOK], 0);
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION during Quick Clean: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Cleaning Error',
        'Exception during cleaning of ' + PluginName + ':' + LineEnding +
        E.Message,
        mtError, [mbOK], 0);
    end;
  end;
end;

procedure TfrmMain.DoCheckITM(Sender: TObject);
var
  PluginIndex: Integer;
  PluginName: string;
  ITMCount: Integer;
  ITMBuf: array[0..4095] of Cardinal;
begin
  PluginIndex := GetSelectedPluginIndex(PluginName);
  if PluginIndex < 0 then
  begin
    MessageDlg('No Plugin Selected',
      'Please select a plugin node in the navigation tree first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if not FEngineReady then
  begin
    MessageDlg('Engine Not Ready',
      'The engine is not initialized. Load a plugin first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  SetStatusText('Checking ' + PluginName + ' for ITM records...');
  LogMessage('Checking for ITM records in ' + PluginName + '...');
  Application.ProcessMessages;

  try
    if not Assigned(xedit_detect_itm) then
    begin
      LogMessage('ERROR: xedit_detect_itm not available in library');
      SetStatusText('Error: Function not available');
      MessageDlg('Function Not Available',
        'The xedit_detect_itm function is not available in the loaded library.',
        mtError, [mbOK], 0);
      Exit;
    end;

    FillChar(ITMBuf, SizeOf(ITMBuf), 0);
    ITMCount := xedit_detect_itm(FEngineHandle, PluginIndex,
      @ITMBuf[0], Length(ITMBuf));

    if ITMCount < 0 then
    begin
      LogMessage('ERROR: xedit_detect_itm returned ' + IntToStr(ITMCount));
      SetStatusText('Error checking for ITMs');
      MessageDlg('Detection Error',
        'Failed to detect ITM records in ' + PluginName + '.' + LineEnding +
        'Error code: ' + IntToStr(ITMCount),
        mtError, [mbOK], 0);
      Exit;
    end;

    LogMessage('Found ' + IntToStr(ITMCount) + ' ITM record(s) in ' + PluginName + '.');
    SetStatusText('Ready - ITM check complete');

    MessageDlg('ITM Check Results',
      PluginName + LineEnding +
      LineEnding +
      'Identical-to-Master records found: ' + IntToStr(ITMCount) + LineEnding +
      LineEnding +
      'Use "Remove ITM Records" or "Quick Clean" to remove them.',
      mtInformation, [mbOK], 0);
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION during ITM check: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Detection Error',
        'Exception during ITM detection:' + LineEnding + E.Message,
        mtError, [mbOK], 0);
    end;
  end;
end;

procedure TfrmMain.DoRemoveITM(Sender: TObject);
var
  PluginIndex: Integer;
  PluginName: string;
  ITMCount: Integer;
begin
  PluginIndex := GetSelectedPluginIndex(PluginName);
  if PluginIndex < 0 then
  begin
    MessageDlg('No Plugin Selected',
      'Please select a plugin node in the navigation tree first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if not FEngineReady then
  begin
    MessageDlg('Engine Not Ready',
      'The engine is not initialized. Load a plugin first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if MessageDlg('Remove ITM Records',
    'Remove Identical-to-Master records from ' + PluginName + '?' + LineEnding +
    LineEnding +
    'This will modify the plugin.',
    mtConfirmation, [mbYes, mbNo], 0) <> mrYes then
    Exit;

  SetStatusText('Removing ITM records from ' + PluginName + '...');
  LogMessage('Removing ITM records from ' + PluginName + '...');
  Application.ProcessMessages;

  try
    if not Assigned(xedit_clean_itm) then
    begin
      LogMessage('ERROR: xedit_clean_itm not available in library');
      SetStatusText('Error: Function not available');
      MessageDlg('Function Not Available',
        'The xedit_clean_itm function is not available in the loaded library.',
        mtError, [mbOK], 0);
      Exit;
    end;

    ITMCount := xedit_clean_itm(FEngineHandle, PluginIndex);
    if ITMCount < 0 then
    begin
      LogMessage('ERROR: xedit_clean_itm returned ' + IntToStr(ITMCount));
      SetStatusText('Error removing ITMs');
      MessageDlg('Cleaning Error',
        'Failed to remove ITM records from ' + PluginName + '.' + LineEnding +
        'Error code: ' + IntToStr(ITMCount),
        mtError, [mbOK], 0);
      Exit;
    end;

    LogMessage('Removed ' + IntToStr(ITMCount) + ' ITM record(s) from ' + PluginName + '.');
    SetStatusText('Ready - ITM removal complete');

    MessageDlg('ITM Removal Complete',
      PluginName + LineEnding +
      LineEnding +
      'ITM records removed: ' + IntToStr(ITMCount),
      mtInformation, [mbOK], 0);
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION during ITM removal: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Cleaning Error',
        'Exception during ITM removal:' + LineEnding + E.Message,
        mtError, [mbOK], 0);
    end;
  end;
end;

procedure TfrmMain.DoUndeleteRefs(Sender: TObject);
var
  PluginIndex: Integer;
  PluginName: string;
  UDRCount: Integer;
begin
  PluginIndex := GetSelectedPluginIndex(PluginName);
  if PluginIndex < 0 then
  begin
    MessageDlg('No Plugin Selected',
      'Please select a plugin node in the navigation tree first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if not FEngineReady then
  begin
    MessageDlg('Engine Not Ready',
      'The engine is not initialized. Load a plugin first.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  if MessageDlg('Undelete and Disable References',
    'Undelete and disable deleted references in ' + PluginName + '?' + LineEnding +
    LineEnding +
    'Deleted references will be undeleted, disabled, and repositioned' + LineEnding +
    'to coordinates 0, 0, -30000.' + LineEnding +
    LineEnding +
    'This will modify the plugin.',
    mtConfirmation, [mbYes, mbNo], 0) <> mrYes then
    Exit;

  SetStatusText('Fixing deleted references in ' + PluginName + '...');
  LogMessage('Undeleting and disabling references in ' + PluginName + '...');
  Application.ProcessMessages;

  try
    if not Assigned(xedit_clean_deleted) then
    begin
      LogMessage('ERROR: xedit_clean_deleted not available in library');
      SetStatusText('Error: Function not available');
      MessageDlg('Function Not Available',
        'The xedit_clean_deleted function is not available in the loaded library.',
        mtError, [mbOK], 0);
      Exit;
    end;

    UDRCount := xedit_clean_deleted(FEngineHandle, PluginIndex);
    if UDRCount < 0 then
    begin
      LogMessage('ERROR: xedit_clean_deleted returned ' + IntToStr(UDRCount));
      SetStatusText('Error fixing deleted references');
      MessageDlg('Cleaning Error',
        'Failed to fix deleted references in ' + PluginName + '.' + LineEnding +
        'Error code: ' + IntToStr(UDRCount),
        mtError, [mbOK], 0);
      Exit;
    end;

    LogMessage('Fixed ' + IntToStr(UDRCount) + ' deleted reference(s) in ' + PluginName + '.');
    SetStatusText('Ready - UDR fix complete');

    MessageDlg('UDR Fix Complete',
      PluginName + LineEnding +
      LineEnding +
      'Deleted references fixed: ' + IntToStr(UDRCount),
      mtInformation, [mbOK], 0);
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION during UDR fix: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('Cleaning Error',
        'Exception during UDR fix:' + LineEnding + E.Message,
        mtError, [mbOK], 0);
    end;
  end;
end;

{ --------------------------------------------------------------------------- }
{ MO2 integration                                                               }
{ --------------------------------------------------------------------------- }

procedure TfrmMain.LoadMO2Folder(const APath: string);
var
  Ret: Integer;
  ProfileCount: Integer;
  SelectedProfile: string;
  PluginCount: Integer;
begin
  { Ensure the FFI library is loaded }
  if not xedit_ffi_loaded then
  begin
    LogMessage('Loading xedit_core library...');
    SetStatusText('Loading xedit_core library...');
    Application.ProcessMessages;

    if not xedit_ffi_load('') then
    begin
      LogMessage('ERROR: Could not load ' + XEDIT_CORE_LIB);
      SetStatusText('Error: xedit_core library not found');
      MessageDlg('Library Not Found',
        'Could not load the xedit_core shared library (' + XEDIT_CORE_LIB + ').' + LineEnding +
        LineEnding +
        'The Rust core library has not been built yet.' + LineEnding +
        'Build it with: cargo build --release',
        mtError, [mbOK], 0);
      Exit;
    end;
    LogMessage('Library loaded successfully.');
  end;

  { Check that MO2 FFI functions are available }
  if not Assigned(xedit_load_mo2) then
  begin
    LogMessage('ERROR: xedit_load_mo2 function not available in library');
    SetStatusText('Error: MO2 support not available in this library build');
    MessageDlg('MO2 Not Supported',
      'The loaded xedit_core library does not include MO2 support.' + LineEnding +
      'Please update to a version that includes MO2 integration.',
      mtError, [mbOK], 0);
    Exit;
  end;

  { Load MO2 configuration from the selected folder }
  SetStatusText('Loading MO2 configuration...');
  LogMessage('Loading MO2 configuration from: ' + APath);
  Application.ProcessMessages;

  try
    Ret := xedit_load_mo2(nil, PChar(APath));
    if Ret < 0 then
    begin
      LogMessage('ERROR: xedit_load_mo2 failed with code ' + IntToStr(Ret));
      SetStatusText('Error: Failed to load MO2 configuration');
      MessageDlg('MO2 Error',
        'Failed to load MO2 configuration from:' + LineEnding +
        APath + LineEnding + LineEnding +
        'Error code: ' + IntToStr(Ret) + LineEnding +
        'Make sure this is a valid MO2 installation folder.',
        mtError, [mbOK], 0);
      Exit;
    end;
    LogMessage('MO2 configuration loaded successfully.');
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION loading MO2: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      MessageDlg('MO2 Error',
        'Exception while loading MO2 configuration:' + LineEnding + E.Message,
        mtError, [mbOK], 0);
      Exit;
    end;
  end;

  { Query available profiles }
  if not Assigned(xedit_mo2_profile_count) then
  begin
    LogMessage('ERROR: xedit_mo2_profile_count not available');
    Exit;
  end;

  ProfileCount := xedit_mo2_profile_count(nil);
  if ProfileCount < 0 then
  begin
    LogMessage('ERROR: xedit_mo2_profile_count returned ' + IntToStr(ProfileCount));
    SetStatusText('Error: Could not query MO2 profiles');
    Exit;
  end;

  if ProfileCount = 0 then
  begin
    LogMessage('Warning: No MO2 profiles found.');
    SetStatusText('No MO2 profiles found');
    MessageDlg('No Profiles',
      'No profiles were found in the MO2 installation.',
      mtWarning, [mbOK], 0);
    Exit;
  end;

  LogMessage('Found ' + IntToStr(ProfileCount) + ' MO2 profile(s).');

  { Show profile selector }
  SelectedProfile := ShowMO2ProfileSelector(ProfileCount);
  if SelectedProfile = '' then
  begin
    LogMessage('MO2 profile selection cancelled.');
    SetStatusText('Ready');
    Exit;
  end;

  LogMessage('Selected MO2 profile: ' + SelectedProfile);

  { Select the profile and build VFS }
  if not Assigned(xedit_mo2_select_profile) then
  begin
    LogMessage('ERROR: xedit_mo2_select_profile not available');
    Exit;
  end;

  SetStatusText('Activating MO2 profile: ' + SelectedProfile + '...');
  LogMessage('Activating MO2 profile: ' + SelectedProfile);
  Application.ProcessMessages;

  try
    Ret := xedit_mo2_select_profile(nil, PChar(SelectedProfile));
    if Ret < 0 then
    begin
      LogMessage('ERROR: xedit_mo2_select_profile failed with code ' + IntToStr(Ret));
      SetStatusText('Error: Failed to select MO2 profile');
      MessageDlg('MO2 Error',
        'Failed to select MO2 profile: ' + SelectedProfile + LineEnding +
        'Error code: ' + IntToStr(Ret),
        mtError, [mbOK], 0);
      Exit;
    end;
    LogMessage('MO2 profile activated and VFS built.');
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION selecting MO2 profile: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      Exit;
    end;
  end;

  { Shut down any existing engine session before MO2 takes over }
  if FEngineReady then
  begin
    ShutdownEngine;
    vstNav.Items.Clear;
  end;

  FMO2Active := True;
  FMO2Profile := SelectedProfile;

  { Initialize the engine - MO2 profile selection configures the game }
  InitEngine;
  if not FEngineReady then
  begin
    LogMessage('ERROR: Engine failed to initialize after MO2 profile selection.');
    FMO2Active := False;
    FMO2Profile := '';
    Exit;
  end;

  { Load all plugins from the MO2 profile load order }
  if not Assigned(xedit_mo2_load_order) then
  begin
    LogMessage('ERROR: xedit_mo2_load_order not available');
    Exit;
  end;

  SetStatusText('Loading plugins from MO2 load order...');
  LogMessage('Loading plugins from MO2 load order...');
  Application.ProcessMessages;

  try
    PluginCount := xedit_mo2_load_order(nil);
    if PluginCount < 0 then
    begin
      LogMessage('ERROR: xedit_mo2_load_order failed with code ' + IntToStr(PluginCount));
      SetStatusText('Error: Failed to load MO2 load order');
      MessageDlg('MO2 Error',
        'Failed to load plugins from MO2 load order.' + LineEnding +
        'Error code: ' + IntToStr(PluginCount),
        mtError, [mbOK], 0);
      Exit;
    end;
    LogMessage('MO2 load order loaded: ' + IntToStr(PluginCount) + ' plugin(s).');
  except
    on E: Exception do
    begin
      LogMessage('EXCEPTION loading MO2 load order: ' + E.Message);
      SetStatusText('Error: ' + E.Message);
      Exit;
    end;
  end;

  { Populate the navigation tree with all loaded plugins }
  PopulateNavFromLoadOrder;

  { Build the referenced-by index asynchronously in a background thread }
  if Assigned(xedit_build_refby_index_async) then
  begin
    SetStatusText('Building Referenced By index (background)...');
    LogMessage('Starting async Referenced By index build...');
    xedit_build_refby_index_async();
    tmrRefByBuild.Enabled := True;
  end
  else if Assigned(xedit_build_refby_index) then
  begin
    { Fallback to synchronous build }
    SetStatusText('Building Referenced By index...');
    LogMessage('Building Referenced By index (sync fallback)...');
    Application.ProcessMessages;
    Ret := xedit_build_refby_index();
    if Ret < 0 then
      LogMessage('WARNING: xedit_build_refby_index returned ' + IntToStr(Ret))
    else
      LogMessage('Referenced By index built successfully.');
  end;

  { Update status bar with MO2 info }
  SetStatusText('MO2: ' + FMO2Profile + ' - ' + GetSelectedGameName);
  LogMessage('MO2 workflow complete. Profile: ' + FMO2Profile +
    ', Game: ' + GetSelectedGameName +
    ', Plugins: ' + IntToStr(PluginCount));
end;

function TfrmMain.ShowMO2ProfileSelector(AProfileCount: Integer): string;
var
  ProfileNames: TStringList;
  I: Integer;
  NameBuf: array[0..511] of Char;
  NameLen: Integer;
  ProfileName: string;
begin
  Result := '';
  ProfileNames := TStringList.Create;
  try
    { Gather all profile names }
    for I := 0 to AProfileCount - 1 do
    begin
      if not Assigned(xedit_mo2_profile_name) then
      begin
        LogMessage('ERROR: xedit_mo2_profile_name not available');
        Exit;
      end;

      FillChar(NameBuf, SizeOf(NameBuf), 0);
      NameLen := xedit_mo2_profile_name(nil, I, @NameBuf[0], SizeOf(NameBuf));
      if NameLen > 0 then
        ProfileName := StrPas(@NameBuf[0])
      else
        ProfileName := '(profile ' + IntToStr(I) + ')';

      ProfileNames.Add(ProfileName);
      LogMessage('  Profile [' + IntToStr(I) + ']: ' + ProfileName);
    end;

    { If only one profile, use it automatically }
    if ProfileNames.Count = 1 then
    begin
      Result := ProfileNames[0];
      LogMessage('Only one profile available, auto-selecting: ' + Result);
      Exit;
    end;

    { Show a selection dialog using InputQuery with the list of profiles }
    ProfileName := ProfileNames[0];
    if not InputQuery('Select MO2 Profile',
      'Available profiles:' + LineEnding +
      ProfileNames.Text + LineEnding +
      'Enter profile name:', ProfileName) then
    begin
      Result := '';  { User cancelled }
      Exit;
    end;

    { Validate the entered profile name }
    if ProfileNames.IndexOf(ProfileName) >= 0 then
      Result := ProfileName
    else
    begin
      { Try case-insensitive match }
      for I := 0 to ProfileNames.Count - 1 do
      begin
        if CompareText(ProfileNames[I], ProfileName) = 0 then
        begin
          Result := ProfileNames[I];
          Exit;
        end;
      end;
      LogMessage('Warning: Profile "' + ProfileName + '" not found in list. Using as-is.');
      Result := ProfileName;
    end;
  finally
    ProfileNames.Free;
  end;
end;

procedure TfrmMain.PopulateNavFromLoadOrder;
var
  TotalPlugins: Integer;
  I, NameLen: Integer;
  PluginName: string;
  NameBuf: array[0..511] of Char;
begin
  if not Assigned(xedit_plugin_count) then
  begin
    LogMessage('Warning: xedit_plugin_count not available, cannot populate tree.');
    Exit;
  end;

  TotalPlugins := xedit_plugin_count();
  if TotalPlugins <= 0 then
  begin
    LogMessage('No plugins loaded from MO2 load order.');
    Exit;
  end;

  LogMessage('Populating navigation tree with ' + IntToStr(TotalPlugins) + ' plugin(s)...');
  SetStatusText('Building navigation tree...');
  Application.ProcessMessages;

  vstNav.Items.Clear;
  vstNav.Items.BeginUpdate;
  try
    for I := 0 to TotalPlugins - 1 do
    begin
      { Get actual plugin filename from Rust }
      PluginName := 'Plugin ' + IntToStr(I);
      if Assigned(xedit_plugin_filename) then
      begin
        FillChar(NameBuf, SizeOf(NameBuf), 0);
        NameLen := xedit_plugin_filename(I, @NameBuf[0], SizeOf(NameBuf));
        if NameLen > 0 then
          PluginName := StrPas(@NameBuf[0]);
      end;

      { Use PopulateNavTree to fill in group details (lazy-loading) }
      PopulateNavTree(I, PluginName);

      { Keep UI responsive during loading - update every 100 plugins }
      if (I mod 100) = 0 then
      begin
        SetStatusText('Building tree... ' + IntToStr(I + 1) + ' / ' + IntToStr(TotalPlugins));
        Application.ProcessMessages;
      end;
    end;
  finally
    vstNav.Items.EndUpdate;
  end;

  LogMessage('Navigation tree populated with ' + IntToStr(TotalPlugins) + ' plugins.');
end;

end.
