{
  Find alternate textures pointing to invalid mesh nodes.
  Supports Fallout 3, New Vegas and Skyrim.
}
unit AlternateTextures;

var
  slNode: TStringList;

function ListNodes(aMesh: string; sl: TStringList): Boolean;
var
  i: integer;
begin
  sl.Clear;
  aMesh := wbNormalizeResourceName(aMesh, resMesh);
  if not ResourceExists(aMesh) then begin
    Result := False;
    Exit;
  end;
  
  NifBlockList(ResourceOpenData('', aMesh), sl);
  for i := Pred(sl.Count) downto 0 do
    if (sl.ValueFromIndex[i] <> 'NiTriShape') and (sl.ValueFromIndex[i] <> 'NiTriStrips') then
      sl.Delete(i);

  for i := 0 to Pred(sl.Count) do begin
    sl[i] := sl.Names[i];
    //AddMessage(Format('Block %d  Name: %s', [i, sl[i]]));
  end;
  
  Result := True;
end;

procedure CheckTextures(e: IInterface; aMODL, aMODS: string);
var
  modl, mods, tex: IInterface;
  i, idx: integer;
  model, node: string;
begin
  modl := ElementByPath(e, aMODL);
  if not Assigned(modl) then
    Exit;

  mods := ElementByPath(e, aMODS);
  if not Assigned(mods) then
    Exit;

  model := GetEditValue(modl);
  if not SameText(ExtractFileExt(model), '.nif') then
    Exit;
    
  if not ListNodes(model, slNode) then begin
    AddMessage(FullPath(modl) + ' file not found: ' + model);
    Exit;
  end;
  
  for i := 0 to Pred(ElementCount(mods)) do begin
    tex := ElementByIndex(mods, i);
    node := GetElementEditValues(tex, '3D Name');
    idx := GetElementNativeValues(tex, '3D Index');
    if (idx >= 0) and (idx < slNode.Count) then
      if slNode[idx] = node then
        Continue;
    AddMessage(Format('%s node %s at index %s not found in "%s" %s', [Name(e), node, IntToStr(idx), aMODL, model]));
  end;
end;

function Initialize: integer;
begin
  slNode := TStringList.Create;
end;

function Process(e: IInterface): integer;
var
  sig: string;
begin
  sig := Signature(e);
  
  // generic model for a record
  if ElementExists(e, 'Model') then
    CheckTextures(e, 'Model\MODL', 'Model\MODS');
   
  // specific ones
  if (sig = 'ARMO') or (sig = 'ARMA') then begin
    CheckTextures(e, 'Male\Biped Model\MODL', 'Male\Biped Model\MODS');
    CheckTextures(e, 'Male\World Model\MOD2', 'Male\World Model\MO2S');
    CheckTextures(e, 'Female\Biped Model\MOD3', 'Female\Biped Model\MO3S');
    CheckTextures(e, 'Female\World Model\MOD4', 'Female\World Model\MO4S');
  end
  else if sig = 'WEAP' then begin
    CheckTextures(e, 'Shell Casing Model\MOD2', 'Shell Casing Model\MO2S');
    CheckTextures(e, 'Scope Model\MOD3', 'Scope Model\MO3S');
    CheckTextures(e, 'World Model\MOD4', 'World Model\MO4S');
  end;
end;

function Finalize: integer;
begin
  slNode.Free;
end;

end.
