import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { AttachedDocument, DraftState, LoadedDraftBundle, SavedDraftSummary } from '../models';

interface NativeDraftFile {
  id: string;
  name: string;
  type: string;
  lastModified: number;
  filePath?: string;
  bytes: number[];
}

interface SaveDraftPayload {
  draft_id?: string | null;
  draft_json: string;
  files: NativeDraftFile[];
}

interface NativeDraftSummary {
  id: string;
  referenceNumber: string;
  updatedAt: string;
  rowCount: number;
  step: number;
  filePath?: string;
}

interface NativeLoadedDraft {
  draftJson: string;
  files: NativeDraftFile[];
}

const LOCAL_KEY = 'insight-data-upload-ui-local-draft';

@Injectable({ providedIn: 'root' })
export class DraftStoreService {
  private get isTauri(): boolean {
    return '__TAURI_INTERNALS__' in window;
  }

  async save(draft: DraftState, documents: AttachedDocument[]): Promise<SavedDraftSummary> {
    const files: NativeDraftFile[] = [];
    for (const document of documents) {
      if (document.filePath) {
        files.push({
          id: document.id,
          name: document.fileName,
          type: document.fileType ?? document.file?.type ?? 'application/octet-stream',
          lastModified: document.lastModified ?? document.file?.lastModified ?? Date.now(),
          filePath: document.filePath,
          bytes: []
        });
        continue;
      }
      if (document.file) {
        files.push({
          id: document.id,
          name: document.file.name,
          type: document.fileType ?? document.file.type,
          lastModified: document.lastModified ?? document.file.lastModified,
          bytes: Array.from(new Uint8Array(await document.file.arrayBuffer()))
        });
      }
      // Metadata-only documents can still be preserved in draft.json. No business data or
      // sensitive file bytes are synthesized when the original file is unavailable.
    }

    const draftJson = JSON.stringify(draft);
    if (this.isTauri) {
      // Tauri commands receive their function arguments by name. The Rust command
      // signature is `save_draft(app, request)`, so the complete payload must be
      // supplied under the `request` argument.
      return invoke<SavedDraftSummary>('save_draft', {
        request: {
          draft_id: draft.draftId ?? null,
          draft_json: draftJson,
          files
        } satisfies SaveDraftPayload
      });
    }

    // Browser-only fallback keeps the existing browser mode usable. The desktop POC uses SQLite + File System.
    localStorage.setItem(LOCAL_KEY, JSON.stringify({ draft, files }));
    return {
      id: draft.draftId ?? 'browser-local',
      referenceNumber: draft.packet.referenceNumber,
      updatedAt: draft.savedAt,
      rowCount: draft.rows.length,
      step: draft.step
    };
  }

  async list(): Promise<SavedDraftSummary[]> {
    if (this.isTauri) {
      return invoke<SavedDraftSummary[]>('list_drafts');
    }
    const local = await this.load();
    if (!local) return [];
    return [{
      id: local.draft.draftId ?? 'browser-local',
      referenceNumber: local.draft.packet.referenceNumber,
      updatedAt: local.draft.savedAt,
      rowCount: local.draft.rows.length,
      step: local.draft.step
    }];
  }

  async loadById(id: string): Promise<LoadedDraftBundle | null> {
    if (this.isTauri) {
      const result = await invoke<NativeLoadedDraft | null>('load_draft', { draftId: id });
      if (!result) return null;
      const draft = JSON.parse(result.draftJson) as DraftState;
      const files: LoadedDraftBundle['files'] = Object.fromEntries(
        result.files.map(file => [file.id, file])
      );
      return { draft, files };
    }
    return this.load();
  }

  async load(): Promise<LoadedDraftBundle | null> {
    if (this.isTauri) {
      const summaries = await this.list();
      const latest = summaries[0];
      return latest ? this.loadById(latest.id) : null;
    }
    const raw = localStorage.getItem(LOCAL_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { draft: DraftState; files: NativeDraftFile[] };
    const files: LoadedDraftBundle['files'] = Object.fromEntries(parsed.files.map(file => [file.id, file]));
    return { draft: parsed.draft, files };
  }

  async remove(id: string): Promise<void> {
    if (this.isTauri) {
      await invoke('delete_draft', { draftId: id });
      return;
    }
    localStorage.removeItem(LOCAL_KEY);
  }

  async getStorageLocation(): Promise<string> {
    if (this.isTauri) return invoke<string>('get_storage_location');
    return 'Browser local storage (fallback mode)';
  }

  async clear(): Promise<void> {
    const summaries = await this.list();
    for (const summary of summaries) await this.remove(summary.id);
  }
}
