import { useState, useCallback } from 'react';
import { Upload, File, X, CheckCircle, AlertCircle, Loader2 } from 'lucide-react';
import { api } from '../api/client';

interface FileUploadProps {
  onUploadComplete: () => void;
}

interface FileStatus {
  file: File;
  status: 'pending' | 'uploading' | 'success' | 'error';
  error?: string;
  chunks?: number;
}

const SUPPORTED_TYPES = [
  '.pdf', '.docx', '.doc', '.pptx', '.txt', '.md', '.xlsx', '.xls',
  '.html', '.htm', '.csv', '.rs', '.py', '.js', '.ts', '.tsx',
  '.jsx', '.go', '.java', '.cpp', '.c', '.h', '.json', '.yaml', '.yml'
];
// Note: .ppt (old PowerPoint) is NOT supported - must be converted to .pptx

export function FileUpload({ onUploadComplete }: FileUploadProps) {
  const [files, setFiles] = useState<FileStatus[]>([]);
  const [isDragging, setIsDragging] = useState(false);
  const [isUploading, setIsUploading] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const droppedFiles = Array.from(e.dataTransfer.files);
    addFiles(droppedFiles);
  }, []);

  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      const selectedFiles = Array.from(e.target.files);
      addFiles(selectedFiles);
    }
  }, []);

  const addFiles = (newFiles: File[]) => {
    const fileStatuses: FileStatus[] = newFiles.map(file => ({
      file,
      status: 'pending',
    }));
    setFiles(prev => [...prev, ...fileStatuses]);
  };

  const removeFile = (index: number) => {
    setFiles(prev => prev.filter((_, i) => i !== index));
  };

  const uploadFiles = async () => {
    const pendingFiles = files.filter(f => f.status === 'pending');
    if (pendingFiles.length === 0) return;

    setIsUploading(true);

    // Mark all as uploading
    setFiles(prev =>
      prev.map(f => (f.status === 'pending' ? { ...f, status: 'uploading' as const } : f))
    );

    try {
      const result = await api.ingest(pendingFiles.map(f => f.file));

      // Update statuses based on result
      setFiles(prev =>
        prev.map(f => {
          if (f.status !== 'uploading') return f;

          const error = result.errors.find(e => e.filename === f.file.name);
          if (error) {
            return { ...f, status: 'error' as const, error: error.error };
          }

          const doc = result.documents.find(d => d.filename === f.file.name);
          return {
            ...f,
            status: 'success' as const,
            chunks: doc?.total_chunks,
          };
        })
      );

      onUploadComplete();
    } catch (error) {
      // Mark all uploading as error
      setFiles(prev =>
        prev.map(f =>
          f.status === 'uploading'
            ? { ...f, status: 'error' as const, error: error instanceof Error ? error.message : 'Upload failed' }
            : f
        )
      );
    } finally {
      setIsUploading(false);
    }
  };

  const clearCompleted = () => {
    setFiles(prev => prev.filter(f => f.status !== 'success'));
  };

  const getFileIcon = (_filename: string) => {
    // Return appropriate icon based on extension
    return <File className="w-5 h-5 text-gray-400" />;
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="space-y-6">
      {/* Drop Zone */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={`border-2 border-dashed rounded-xl p-8 text-center transition-colors ${
          isDragging
            ? 'border-primary-500 bg-primary-50'
            : 'border-gray-300 hover:border-gray-400'
        }`}
      >
        <Upload
          className={`w-12 h-12 mx-auto mb-4 ${
            isDragging ? 'text-primary-500' : 'text-gray-400'
          }`}
        />
        <p className="text-lg font-medium text-gray-700 mb-1">
          {isDragging ? 'Drop files here' : 'Drag and drop files'}
        </p>
        <p className="text-sm text-gray-500 mb-4">
          or click to browse
        </p>
        <input
          type="file"
          multiple
          accept={SUPPORTED_TYPES.join(',')}
          onChange={handleFileSelect}
          className="hidden"
          id="file-input"
        />
        <label
          htmlFor="file-input"
          className="inline-flex items-center px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 cursor-pointer transition-colors"
        >
          Select Files
        </label>
        <p className="text-xs text-gray-400 mt-4">
          Supported: PDF, DOCX, PPTX, TXT, MD, XLSX, HTML, CSV, and code files
          <br />
          <span className="text-yellow-500">Note: Old .ppt files must be converted to .pptx</span>
        </p>
      </div>

      {/* File List */}
      {files.length > 0 && (
        <div className="bg-white rounded-xl border border-gray-200 divide-y divide-gray-100">
          <div className="px-4 py-3 flex items-center justify-between">
            <h3 className="font-medium text-gray-900">
              {files.length} file{files.length !== 1 ? 's' : ''} selected
            </h3>
            <div className="flex items-center gap-2">
              {files.some(f => f.status === 'success') && (
                <button
                  onClick={clearCompleted}
                  className="text-sm text-gray-500 hover:text-gray-700"
                >
                  Clear completed
                </button>
              )}
            </div>
          </div>

          <div className="divide-y divide-gray-100 max-h-80 overflow-y-auto">
            {files.map((fileStatus, index) => (
              <div
                key={`${fileStatus.file.name}-${index}`}
                className="px-4 py-3 flex items-center gap-4"
              >
                {getFileIcon(fileStatus.file.name)}
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-gray-900 truncate">
                    {fileStatus.file.name}
                  </p>
                  <p className="text-xs text-gray-500">
                    {formatFileSize(fileStatus.file.size)}
                    {fileStatus.chunks && ` • ${fileStatus.chunks} chunks`}
                  </p>
                  {fileStatus.error && (
                    <p className="text-xs text-red-500 mt-1">{fileStatus.error}</p>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  {fileStatus.status === 'pending' && (
                    <button
                      onClick={() => removeFile(index)}
                      className="p-1 text-gray-400 hover:text-gray-600"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  )}
                  {fileStatus.status === 'uploading' && (
                    <Loader2 className="w-5 h-5 text-primary-500 animate-spin" />
                  )}
                  {fileStatus.status === 'success' && (
                    <CheckCircle className="w-5 h-5 text-green-500" />
                  )}
                  {fileStatus.status === 'error' && (
                    <AlertCircle className="w-5 h-5 text-red-500" />
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Upload Button */}
          {files.some(f => f.status === 'pending') && (
            <div className="px-4 py-3">
              <button
                onClick={uploadFiles}
                disabled={isUploading}
                className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
              >
                {isUploading ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Uploading...
                  </>
                ) : (
                  <>
                    <Upload className="w-4 h-4" />
                    Upload {files.filter(f => f.status === 'pending').length} file
                    {files.filter(f => f.status === 'pending').length !== 1 ? 's' : ''}
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
