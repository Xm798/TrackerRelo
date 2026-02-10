import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardAction,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import {
  ArrowRight,
  CheckCircle2,
  XCircle,
  Loader2,
  Plus,
  Trash2,
  Plug,
  Replace,
  Search,
  Play,
  ChevronDown,
  ChevronRight,
  Lock,
  List,
  ArrowDownToLine,
} from "lucide-react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ConnectionConfig {
  downloader_type: string;
  host: string;
  port: number;
  username: string;
  password: string;
  use_https: boolean;
}

interface Rule {
  old_domain: string;
  new_domain: string;
  enabled: boolean;
}

interface AppConfig {
  connection: ConnectionConfig;
  rules: Rule[];
}

interface ScanResult {
  total_torrents: number;
  matched_torrents: number;
  matches: MatchedTorrent[];
}

interface MatchedTorrent {
  hash: string;
  name: string;
  old_url: string;
  new_url: string;
}

interface ReplaceResult {
  torrent_name: string;
  old_url: string;
  new_url: string;
  success: boolean;
  error: string | null;
}

interface TrackerEntry {
  domain: string;
  count: number;
}

type ConnectionStatus = "idle" | "testing" | "connected" | "error";

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

function App() {
  // -- Connection state
  const [connection, setConnection] = useState<ConnectionConfig>({
    downloader_type: "qbittorrent",
    host: "127.0.0.1",
    port: 8080,
    username: "admin",
    password: "",
    use_https: false,
  });
  const [connectionStatus, setConnectionStatus] =
    useState<ConnectionStatus>("idle");
  const [connectionMessage, setConnectionMessage] = useState("");

  // -- Rules state
  const [rules, setRules] = useState<Rule[]>([
    { old_domain: "", new_domain: "", enabled: true },
  ]);

  // -- Preview state
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [showMatches, setShowMatches] = useState(false);

  // -- Execution state
  const [isExecuting, setIsExecuting] = useState(false);

  // -- Updater state
  const [appVersion, setAppVersion] = useState("");
  const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const updateChecked = useRef(false);

  // -- Tracker browser state
  const [existingTrackers, setExistingTrackers] = useState<TrackerEntry[]>([]);
  const [isFetchingTrackers, setIsFetchingTrackers] = useState(false);
  const [trackerPopoverOpen, setTrackerPopoverOpen] = useState(false);

  // -- Load config on mount
  useEffect(() => {
    invoke<AppConfig>("load_config")
      .then((cfg) => {
        setConnection(cfg.connection);
        if (cfg.rules.length > 0) setRules(cfg.rules);
      })
      .catch(() => {
        // Use defaults if load fails
      });
  }, []);

  // -- Fetch version and check for updates on mount
  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => {});
    if (updateChecked.current) return;
    updateChecked.current = true;
    check().then((update) => {
      if (update?.available) setUpdateAvailable(update);
    }).catch(() => {});
  }, []);

  // -- Handle update install
  const handleUpdate = async () => {
    if (!updateAvailable) return;
    setIsUpdating(true);
    try {
      await updateAvailable.downloadAndInstall();
      await relaunch();
    } catch {
      setIsUpdating(false);
    }
  };

  // -- Persist config helper
  const buildConfig = useCallback(
    (): AppConfig => ({ connection, rules }),
    [connection, rules],
  );

  const saveConfig = useCallback(async () => {
    try {
      await invoke("save_config", { config: buildConfig() });
    } catch {
      // Silent save failure
    }
  }, [buildConfig]);

  // -- Auto-save on config changes (debounced)
  useEffect(() => {
    const timer = setTimeout(() => {
      saveConfig();
    }, 800);
    return () => clearTimeout(timer);
  }, [saveConfig]);

  // -- Connection field updater
  const defaultPort = (client: string) =>
    client === "transmission" ? 9091 : 8080;

  const updateConnection = <K extends keyof ConnectionConfig>(
    key: K,
    value: ConnectionConfig[K],
  ) => {
    setConnectionStatus("idle");
    setConnectionMessage("");
    setConnection((prev) => {
      const next = { ...prev, [key]: value };
      if (key === "downloader_type") {
        next.port = next.use_https ? 443 : defaultPort(value as string);
      }
      if (key === "use_https") {
        next.port = value ? 443 : defaultPort(next.downloader_type);
      }
      return next;
    });
  };

  // -- Test connection
  const handleTestConnection = async () => {
    setConnectionStatus("testing");
    setConnectionMessage("");
    try {
      const msg = await invoke<string>("test_connection", { connection });
      setConnectionStatus("connected");
      setConnectionMessage(msg);
    } catch (err) {
      setConnectionStatus("error");
      setConnectionMessage(String(err));
    }
  };

  // -- Rule management
  const updateRule = <K extends keyof Rule>(
    index: number,
    key: K,
    value: Rule[K],
  ) => {
    setRules((prev) =>
      prev.map((r, i) => (i === index ? { ...r, [key]: value } : r)),
    );
  };

  const addRule = () => {
    setRules((prev) => [
      ...prev,
      { old_domain: "", new_domain: "", enabled: true },
    ]);
  };

  const removeRule = (index: number) => {
    setRules((prev) => (prev.length <= 1 ? prev : prev.filter((_, i) => i !== index)));
  };

  // -- Fetch existing trackers
  const handleFetchTrackers = async () => {
    setIsFetchingTrackers(true);
    try {
      const entries = await invoke<TrackerEntry[]>("list_trackers", { connection });
      setExistingTrackers(entries);
      setTrackerPopoverOpen(true);
    } catch {
      // Fetch failed — popover stays closed
    } finally {
      setIsFetchingTrackers(false);
    }
  };

  const addRuleFromTracker = (domain: string) => {
    // If there's an empty first rule, fill it in instead of appending
    const emptyIdx = rules.findIndex((r) => r.old_domain.trim() === "" && r.new_domain.trim() === "");
    if (emptyIdx !== -1) {
      updateRule(emptyIdx, "old_domain", domain);
    } else {
      setRules((prev) => [...prev, { old_domain: domain, new_domain: "", enabled: true }]);
    }
    setTrackerPopoverOpen(false);
  };

  // -- Scan
  const handleScan = async () => {
    setIsScanning(true);
    setScanResult(null);
    setShowMatches(false);
    try {
      const result = await invoke<ScanResult>("scan_torrents", {
        config: buildConfig(),
      });
      setScanResult(result);
    } catch {
      // Scan failed
    } finally {
      setIsScanning(false);
    }
  };

  // -- Execute
  const handleExecute = async () => {
    setIsExecuting(true);
    try {
      await invoke<ReplaceResult[]>("execute_replace", {
        config: buildConfig(),
      });
      setScanResult(null);
    } catch {
      // Execute failed
    } finally {
      setIsExecuting(false);
    }
  };

  // -- Derived
  const activeRuleCount = rules.filter(
    (r) => r.enabled && r.old_domain.trim() !== "",
  ).length;
  const isConnected = connectionStatus === "connected";
  const isBusy = isScanning || isExecuting;

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      {/* Title bar / drag region */}
      <div
        data-tauri-drag-region
        className="flex h-[52px] shrink-0 select-none items-center border-b px-5"
      >
        <div className="flex items-center gap-2.5 pl-[68px]" data-tauri-drag-region>
          <Replace className="size-4 text-blue-500" />
          <span className="text-sm font-semibold tracking-tight" data-tauri-drag-region>
            TrackerRelo
          </span>
          <span className="text-xs text-muted-foreground" data-tauri-drag-region>
            {appVersion ? `v${appVersion}` : ""}
          </span>
          {updateAvailable && (
            <button
              onClick={handleUpdate}
              disabled={isUpdating}
              className="flex items-center gap-1 rounded-full border border-blue-200 bg-blue-50 px-2 py-0.5 text-[10px] font-medium text-blue-700 transition-colors hover:bg-blue-100 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-400 dark:hover:bg-blue-900"
            >
              {isUpdating ? (
                <Loader2 className="size-2.5 animate-spin" />
              ) : (
                <ArrowDownToLine className="size-2.5" />
              )}
              {isUpdating ? "Updating..." : `v${updateAvailable.version}`}
            </button>
          )}
        </div>
      </div>

      {/* Scrollable body */}
      <ScrollArea className="flex-1">
        <div className="flex flex-col gap-3 p-4">
          {/* ============================================================= */}
          {/* 1. Connection Config                                           */}
          {/* ============================================================= */}
          <Card className="gap-0 border-border/60 py-0 shadow-none">
            <CardHeader className="px-4 py-3">
              <CardTitle className="flex items-center gap-2 text-[13px]">
                <Plug className="size-3.5 text-muted-foreground" />
                Connection
              </CardTitle>
              <CardAction>
                <ConnectionStatusBadge
                  status={connectionStatus}
                  message={connectionMessage}
                />
              </CardAction>
            </CardHeader>
            <Separator />
            <CardContent className="px-4 py-3">
              <div className="grid grid-cols-[1fr_1.5fr_0.6fr] gap-x-3 gap-y-2.5">
                {/* Row 1 */}
                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">Client</Label>
                  <Select
                    value={connection.downloader_type}
                    onValueChange={(v) => updateConnection("downloader_type", v)}
                  >
                    <SelectTrigger className="h-8 w-full text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="qbittorrent">qBittorrent</SelectItem>
                      <SelectItem value="transmission">Transmission</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1">
                  <div className="flex items-center justify-between">
                    <Label className="text-xs text-muted-foreground">Host</Label>
                    <label className="flex cursor-pointer items-center gap-1.5">
                      <Lock className="size-2.5 text-muted-foreground/60" />
                      <span className="text-[10px] text-muted-foreground">HTTPS</span>
                      <Switch
                        size="sm"
                        checked={connection.use_https}
                        onCheckedChange={(checked) =>
                          updateConnection("use_https", checked === true)
                        }
                      />
                    </label>
                  </div>
                  <Input
                    className="h-8 font-mono text-xs"
                    value={connection.host}
                    onChange={(e) => updateConnection("host", e.target.value)}
                    placeholder="127.0.0.1"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">Port</Label>
                  <Input
                    className="h-8 font-mono text-xs"
                    type="number"
                    value={connection.port}
                    onChange={(e) =>
                      updateConnection("port", parseInt(e.target.value) || 0)
                    }
                  />
                </div>

                {/* Row 2 */}
                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">Username</Label>
                  <Input
                    className="h-8 text-xs"
                    value={connection.username}
                    onChange={(e) => updateConnection("username", e.target.value)}
                    placeholder="admin"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">Password</Label>
                  <Input
                    className="h-8 text-xs"
                    type="password"
                    value={connection.password}
                    onChange={(e) => updateConnection("password", e.target.value)}
                  />
                </div>
                <div className="flex items-end">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 w-full gap-1.5 text-xs"
                    onClick={handleTestConnection}
                    disabled={connectionStatus === "testing"}
                  >
                    {connectionStatus === "testing" ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : (
                      <Plug className="size-3" />
                    )}
                    Test
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* ============================================================= */}
          {/* 2. Replacement Rules                                           */}
          {/* ============================================================= */}
          <Card className="gap-0 border-border/60 py-0 shadow-none">
            <CardHeader className="px-4 py-3">
              <CardTitle className="flex items-center gap-2 text-[13px]">
                <Replace className="size-3.5 text-muted-foreground" />
                Replacement Rules
              </CardTitle>
              <CardAction className="flex items-center gap-1.5">
                {activeRuleCount > 0 && (
                  <Badge variant="secondary" className="font-mono text-[10px]">
                    {activeRuleCount} active
                  </Badge>
                )}
                <Popover open={trackerPopoverOpen} onOpenChange={setTrackerPopoverOpen}>
                  <PopoverTrigger asChild>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 gap-1.5 text-xs"
                      onClick={handleFetchTrackers}
                      disabled={isFetchingTrackers}
                    >
                      {isFetchingTrackers ? (
                        <Loader2 className="size-3 animate-spin" />
                      ) : (
                        <List className="size-3" />
                      )}
                      Browse
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="end" className="w-80 overflow-hidden p-0">
                    <div className="px-3 py-2 border-b">
                      <p className="text-xs font-medium">Existing Trackers</p>
                      <p className="text-[10px] text-muted-foreground">
                        Select a tracker to create a rule
                      </p>
                    </div>
                    <div className="max-h-[240px] overflow-y-auto">
                      {existingTrackers.length === 0 ? (
                        <div className="flex items-center justify-center py-6 text-xs text-muted-foreground">
                          No trackers found
                        </div>
                      ) : (
                        <div className="py-1">
                          {existingTrackers.map((t) => {
                            const alreadyUsed = rules.some(
                              (r) => r.old_domain === t.domain,
                            );
                            return (
                              <button
                                key={t.domain}
                                className="flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-xs transition-colors hover:bg-accent disabled:opacity-40 disabled:pointer-events-none"
                                onClick={() => addRuleFromTracker(t.domain)}
                                disabled={alreadyUsed}
                              >
                                <span className="truncate font-mono">
                                  {t.domain}
                                </span>
                                <Badge
                                  variant="secondary"
                                  className="shrink-0 font-mono text-[10px]"
                                >
                                  {t.count}
                                </Badge>
                              </button>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  </PopoverContent>
                </Popover>
              </CardAction>
            </CardHeader>
            <Separator />
            <CardContent className="px-4 py-3">
              <div className="space-y-2">
                {/* Column headers */}
                <div className="grid grid-cols-[32px_1fr_24px_1fr_32px] items-center gap-2 px-0.5">
                  <span />
                  <span className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                    Old domain
                  </span>
                  <span />
                  <span className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                    New domain
                  </span>
                  <span />
                </div>

                {/* Rule rows */}
                {rules.map((rule, i) => (
                  <div
                    key={i}
                    className="group grid grid-cols-[32px_1fr_24px_1fr_32px] items-center gap-2"
                  >
                    <div className="flex justify-center">
                      <Switch
                        size="sm"
                        checked={rule.enabled}
                        onCheckedChange={(checked) =>
                          updateRule(i, "enabled", checked === true)
                        }
                      />
                    </div>
                    <Input
                      className="h-8 font-mono text-xs"
                      placeholder="old.tracker.com"
                      value={rule.old_domain}
                      onChange={(e) =>
                        updateRule(i, "old_domain", e.target.value)
                      }
                    />
                    <div className="flex justify-center">
                      <ArrowRight className="size-3.5 text-muted-foreground/50" />
                    </div>
                    <Input
                      className="h-8 font-mono text-xs"
                      placeholder="new.tracker.com"
                      value={rule.new_domain}
                      onChange={(e) =>
                        updateRule(i, "new_domain", e.target.value)
                      }
                    />
                    <div className="flex justify-center">
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        className="text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
                        onClick={() => removeRule(i)}
                        disabled={rules.length <= 1}
                      >
                        <Trash2 className="size-3" />
                      </Button>
                    </div>
                  </div>
                ))}

                {/* Add rule button */}
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-full gap-1.5 text-xs text-muted-foreground hover:text-foreground"
                  onClick={addRule}
                >
                  <Plus className="size-3" />
                  Add rule
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* ============================================================= */}
          {/* 3. Preview & Execute                                           */}
          {/* ============================================================= */}
          <Card className="gap-0 border-border/60 py-0 shadow-none">
            <CardHeader className="px-4 py-3">
              <CardTitle className="flex items-center gap-2 text-[13px]">
                <Search className="size-3.5 text-muted-foreground" />
                Preview
              </CardTitle>
              <CardAction className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 text-xs"
                  onClick={handleScan}
                  disabled={isBusy || !isConnected || activeRuleCount === 0}
                >
                  {isScanning ? (
                    <Loader2 className="size-3 animate-spin" />
                  ) : (
                    <Search className="size-3" />
                  )}
                  Scan
                </Button>
                <Button
                  size="sm"
                  className="h-7 gap-1.5 bg-blue-600 text-xs text-white hover:bg-blue-700 dark:bg-blue-600 dark:hover:bg-blue-500"
                  onClick={handleExecute}
                  disabled={
                    isBusy ||
                    !isConnected ||
                    !scanResult ||
                    scanResult.matched_torrents === 0
                  }
                >
                  {isExecuting ? (
                    <Loader2 className="size-3 animate-spin" />
                  ) : (
                    <Play className="size-3" />
                  )}
                  Execute
                </Button>
              </CardAction>
            </CardHeader>
            <Separator />
            <CardContent className="px-4 py-3">
              {!scanResult ? (
                <div className="flex items-center justify-center py-6 text-xs text-muted-foreground">
                  Run a scan to preview matched torrents
                </div>
              ) : (
                <div className="space-y-2.5">
                  {/* Stats bar */}
                  <div className="flex items-center gap-3">
                    <div className="flex items-baseline gap-1.5">
                      <span className="font-mono text-2xl font-semibold tabular-nums leading-none">
                        {scanResult.matched_torrents}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        matched
                      </span>
                    </div>
                    <span className="text-xs text-muted-foreground/40">/</span>
                    <div className="flex items-baseline gap-1.5">
                      <span className="font-mono text-lg tabular-nums leading-none text-muted-foreground">
                        {scanResult.total_torrents}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        total
                      </span>
                    </div>
                    {scanResult.matched_torrents > 0 && (
                      <Badge
                        variant="secondary"
                        className="ml-auto font-mono text-[10px]"
                      >
                        {Math.round(
                          (scanResult.matched_torrents /
                            scanResult.total_torrents) *
                            100,
                        )}
                        %
                      </Badge>
                    )}
                  </div>

                  {/* Expandable match list */}
                  {scanResult.matches.length > 0 && (
                    <div>
                      <button
                        onClick={() => setShowMatches(!showMatches)}
                        className="flex w-full items-center gap-1.5 rounded-md px-1 py-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
                      >
                        {showMatches ? (
                          <ChevronDown className="size-3" />
                        ) : (
                          <ChevronRight className="size-3" />
                        )}
                        {scanResult.matches.length} tracker{" "}
                        {scanResult.matches.length === 1
                          ? "replacement"
                          : "replacements"}
                      </button>

                      {showMatches && (
                        <div className="mt-1.5 max-h-[180px] overflow-y-auto">
                          <div className="space-y-px">
                            {scanResult.matches.map((m, i) => (
                              <div
                                key={`${m.hash}-${i}`}
                                className="rounded-md bg-muted/50 px-3 py-2"
                              >
                                <div className="mb-1 truncate text-xs font-medium">
                                  {m.name}
                                </div>
                                <div className="min-w-0 space-y-0.5 font-mono text-[11px]">
                                  <div className="truncate text-muted-foreground">
                                    {m.old_url}
                                  </div>
                                  <div className="flex items-center gap-1 text-foreground">
                                    <ArrowRight className="size-2.5 shrink-0 text-muted-foreground/40" />
                                    <span className="truncate">{m.new_url}</span>
                                  </div>
                                </div>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}
            </CardContent>
          </Card>

        </div>
      </ScrollArea>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ConnectionStatusBadge({
  status,
  message,
}: {
  status: ConnectionStatus;
  message: string;
}) {
  if (status === "idle") return null;

  if (status === "testing") {
    return (
      <Badge variant="secondary" className="gap-1.5 font-mono text-[10px]">
        <Loader2 className="size-2.5 animate-spin" />
        Testing...
      </Badge>
    );
  }

  if (status === "connected") {
    return (
      <Badge
        variant="secondary"
        className="gap-1.5 border-emerald-200 bg-emerald-50 font-mono text-[10px] text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-400"
      >
        <CheckCircle2 className="size-2.5" />
        {message || "Connected"}
      </Badge>
    );
  }

  return (
    <Badge
      variant="secondary"
      className="max-w-[260px] gap-1.5 truncate border-red-200 bg-red-50 font-mono text-[10px] text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-400"
    >
      <XCircle className="size-2.5 shrink-0" />
      <span className="truncate">{message || "Failed"}</span>
    </Badge>
  );
}

export default App;
