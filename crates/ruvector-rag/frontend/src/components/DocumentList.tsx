import { useState } from 'react';
import {
  FileText,
  Trash2,
  RefreshCw,
  FileSpreadsheet,
  FileCode,
  File,
  Calendar,
  Hash,
  HardDrive,
} from 'lucide-react';
import type { Document } from '../api/types';

interface DocumentListProps {
  documents: Document[];
  onDelete: (id: string) => Promise<void>;
  onRefresh: () => void;
}

export function DocumentList({ documents, onDelete, onRefresh }: DocumentListProps) {
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const handleDelete = async (id: string, filename: string) => {
    if (!confirm(`Delete "${filename}" and all its chunks?`)) return;

    setDeletingId(id);
    try {
      await onDelete(id);
    } finally {
      setDeletingId(null);
    }
  };

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await new Promise(resolve => setTimeout(resolve, 500)); // Min visual feedback
    onRefresh();
    setIsRefreshing(false);
  };

  const getFileIcon = (fileType: string) => {
    const type = fileType.toLowerCase();
    if (type === 'pdf' || type === 'docx') return <FileText className="w-5 h-5 text-red-500" />;
    if (type === 'xlsx' || type === 'csv') return <FileSpreadsheet className="w-5 h-5 text-green-500" />;
    if (type.startsWith('code') || type === 'txt' || type === 'markdown')
      return <FileCode className="w-5 h-5 text-blue-500" />;
    if (type === 'html') return <FileCode className="w-5 h-5 text-orange-500" />;
    return <File className="w-5 h-5 text-gray-500" />;
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const getFileTypeBadge = (fileType: string) => {
    const type = fileType.toLowerCase();
    const colors: Record<string, string> = {
      pdf: 'bg-red-100 text-red-700',
      docx: 'bg-blue-100 text-blue-700',
      txt: 'bg-gray-100 text-gray-700',
      markdown: 'bg-purple-100 text-purple-700',
      xlsx: 'bg-green-100 text-green-700',
      csv: 'bg-emerald-100 text-emerald-700',
      html: 'bg-orange-100 text-orange-700',
    };

    const colorClass = type.startsWith('code')
      ? 'bg-indigo-100 text-indigo-700'
      : colors[type] || 'bg-gray-100 text-gray-700';

    return (
      <span className={`text-xs px-2 py-0.5 rounded ${colorClass}`}>
        {type.startsWith('code') ? type.replace('code(', '').replace(')', '') : type.toUpperCase()}
      </span>
    );
  };

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900">Documents</h2>
          <p className="text-sm text-gray-500">
            {documents.length} document{documents.length !== 1 ? 's' : ''} indexed
          </p>
        </div>
        <button
          onClick={handleRefresh}
          disabled={isRefreshing}
          className="flex items-center gap-2 px-3 py-2 text-sm text-gray-600 hover:text-gray-900 hover:bg-gray-100 rounded-lg transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${isRefreshing ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {/* Document List */}
      {documents.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 p-8 text-center">
          <FileText className="w-12 h-12 text-gray-300 mx-auto mb-4" />
          <h3 className="text-lg font-medium text-gray-700 mb-2">No documents yet</h3>
          <p className="text-sm text-gray-500">
            Upload documents to start asking questions about their content.
          </p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 divide-y divide-gray-100">
          {documents.map(doc => (
            <div
              key={doc.id}
              className="p-4 flex items-start gap-4 hover:bg-gray-50 transition-colors"
            >
              <div className="flex-shrink-0 p-2 bg-gray-100 rounded-lg">
                {getFileIcon(doc.file_type)}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <h3 className="font-medium text-gray-900 truncate">{doc.filename}</h3>
                  {getFileTypeBadge(doc.file_type)}
                </div>
                <div className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-gray-500">
                  <span className="flex items-center gap-1">
                    <Hash className="w-3 h-3" />
                    {doc.total_chunks} chunks
                  </span>
                  {doc.total_pages && (
                    <span className="flex items-center gap-1">
                      <FileText className="w-3 h-3" />
                      {doc.total_pages} pages
                    </span>
                  )}
                  <span className="flex items-center gap-1">
                    <HardDrive className="w-3 h-3" />
                    {formatFileSize(doc.file_size)}
                  </span>
                  <span className="flex items-center gap-1">
                    <Calendar className="w-3 h-3" />
                    {formatDate(doc.ingested_at)}
                  </span>
                </div>
              </div>
              <button
                onClick={() => handleDelete(doc.id, doc.filename)}
                disabled={deletingId === doc.id}
                className="flex-shrink-0 p-2 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors disabled:opacity-50"
                title="Delete document"
              >
                <Trash2 className={`w-4 h-4 ${deletingId === doc.id ? 'animate-pulse' : ''}`} />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Stats */}
      {documents.length > 0 && (
        <div className="grid grid-cols-3 gap-4">
          <div className="bg-white rounded-lg border border-gray-200 p-4">
            <p className="text-2xl font-bold text-gray-900">{documents.length}</p>
            <p className="text-sm text-gray-500">Documents</p>
          </div>
          <div className="bg-white rounded-lg border border-gray-200 p-4">
            <p className="text-2xl font-bold text-gray-900">
              {documents.reduce((sum, d) => sum + d.total_chunks, 0)}
            </p>
            <p className="text-sm text-gray-500">Total Chunks</p>
          </div>
          <div className="bg-white rounded-lg border border-gray-200 p-4">
            <p className="text-2xl font-bold text-gray-900">
              {formatFileSize(documents.reduce((sum, d) => sum + d.file_size, 0))}
            </p>
            <p className="text-sm text-gray-500">Total Size</p>
          </div>
        </div>
      )}
    </div>
  );
}
