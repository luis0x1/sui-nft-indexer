-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "object_types" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateIndex
CREATE INDEX "objects_status_idx" ON "objects"("status");
