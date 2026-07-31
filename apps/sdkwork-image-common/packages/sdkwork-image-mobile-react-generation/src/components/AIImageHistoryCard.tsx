import React from "react";
import { Trash2 } from "lucide-react";
import { ImageTask, AIImageOptions } from "../services/AIImageService";

interface AIImageHistoryCardProps {
  item: ImageTask;
  onSelect: (item: ImageTask) => void;
  onDelete: (e: React.MouseEvent, id: string) => void;
}

export const AIImageHistoryCard: React.FC<AIImageHistoryCardProps> = ({
  item,
  onSelect,
  onDelete,
}) => {
  return (
    <div
      className="rounded-xl overflow-hidden border border-border-color relative cursor-pointer group shadow-sm bg-bg-color"
      style={{
        aspectRatio: item.options.aspectRatio.replace(":", "/"),
      }}
      onClick={() => onSelect(item)}
    >
      <img
        src={item.imageUrl}
        alt={item.options.prompt}
        className="w-full h-full object-cover group-active:scale-[1.02] transition-transform duration-300"
      />
      <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/10 to-transparent opacity-0 group-hover:opacity-100 group-active:opacity-100 transition-opacity flex flex-col justify-end p-2.5">
        <span className="text-[9px] font-medium text-white/80 uppercase tracking-wider mb-0.5">
          {item.options.style}
        </span>
        <p className="text-[11px] text-white line-clamp-2 leading-tight">
          {item.options.prompt}
        </p>
      </div>
      <button
        onClick={(e) => onDelete(e, item.id)}
        className="absolute top-1.5 right-1.5 bg-black/40 p-1.5 rounded-full text-white/80 hover:bg-black/80 hover:text-red-400 active:text-red-400 active:bg-black/80 backdrop-blur z-10 opacity-0 group-hover:opacity-100 group-active:opacity-100 transition-opacity"
      >
        <Trash2 className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};
